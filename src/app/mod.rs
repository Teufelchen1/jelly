use std::collections::HashMap;
use std::sync::mpsc::Sender;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::Packet;
use coap_lite::RequestType as Method;
use rand::Rng;
use ratatui::text::Text;
use slipmux::encode_buffered;
use slipmux::Slipmux;
use tui_widgets::scrollview::ScrollViewState;

use crate::commands::CommandHandler;
use crate::commands::CommandLibrary;
use crate::events::Event;

mod handler;
mod tui;

enum SaveToFile {
    No,
    AsBin(String),
    AsText(String),
}

struct Job {
    handler: Box<dyn CommandHandler>,
    file: SaveToFile,
}

#[derive(Default, Clone, Copy)]
enum SelectedTab {
    #[default]
    Combined,
    Diagnostic,
    Configuration,
    Help,
}

pub struct App<'text> {
    event_sender: Sender<Event>,
    write_port: Option<String>,
    configuration_requests: Vec<CoapRequest<String>>,
    configuration_packets: Vec<Packet>,
    configuration_scroll_state: ScrollViewState,
    configuration_scroll_follow: bool,
    diagnostic_messages: Text<'text>,
    diagnostic_messages_scroll_state: ScrollViewState,
    diagnostic_messages_scroll_position: usize,
    diagnostic_messages_scroll_follow: bool,
    current_tab: SelectedTab,
    known_commands: CommandLibrary,
    user_input: String,
    user_command_history: Vec<String>,
    user_command_cursor: usize,
    token_count: u16,
    riot_board: String,
    riot_version: String,
    next_mid: u16,
    jobs: HashMap<u64, Job>,
}

impl App<'_> {
    pub fn new(event_sender: Sender<Event>) -> Self {
        Self {
            event_sender,
            write_port: None,
            configuration_requests: vec![],
            configuration_packets: vec![],
            configuration_scroll_state: ScrollViewState::default(),
            configuration_scroll_follow: true,
            diagnostic_messages: Text::default(),
            diagnostic_messages_scroll_state: ScrollViewState::default(),
            diagnostic_messages_scroll_position: 0,
            diagnostic_messages_scroll_follow: true,
            current_tab: SelectedTab::Combined,
            known_commands: CommandLibrary::default(),
            user_input: String::new(),
            user_command_history: vec![],
            user_command_cursor: 0,
            token_count: 0,
            riot_board: "Unkown".to_owned(),
            riot_version: "Unkown".to_owned(),

            next_mid: rand::rng().random(),

            jobs: HashMap::new(),
        }
    }

    fn get_new_token(&mut self) -> Vec<u8> {
        self.token_count += 1;
        self.token_count.to_le_bytes().to_vec()
    }

    const fn get_new_message_id(&mut self) -> u16 {
        self.next_mid = self.next_mid.wrapping_add(1);
        self.next_mid
    }

    fn build_get_request(path: &str) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(path);
        request
    }

    fn send_configuration_request(&mut self, msg: &mut Packet) {
        msg.header.message_id = self.get_new_message_id();
        msg.set_token(self.get_new_token());
        msg.add_option(CoapOption::Block2, vec![0x05]);

        let data = encode_buffered(Slipmux::Configuration(msg.to_bytes().unwrap()));
        self.event_sender
            .send(Event::SendConfiguration(data))
            .unwrap();
    }
}
