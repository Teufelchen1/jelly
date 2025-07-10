use std::collections::HashMap;
use std::sync::mpsc::Sender;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::Packet;
use coap_lite::RequestType as Method;
use datatypes::DiagnosticLog;
use datatypes::JobLog;
use rand::Rng;
use slipmux::encode_buffered;
use slipmux::Slipmux;
use tui_widgets::scrollview::ScrollViewState;

use crate::app::datatypes::Job;
use crate::app::datatypes::Request;
use crate::commands::CommandLibrary;
use crate::events::Event;

mod datatypes;
mod handler;
mod tui;

#[derive(Default, Clone, Copy)]
enum SelectedTab {
    #[default]
    Combined,
    Diagnostic,
    Configuration,
    Commands,
    Help,
}

struct UiState {
    device_path: Option<String>,
    diagnostic_messages_scroll_state: ScrollViewState,
    diagnostic_messages_scroll_position: usize,
    diagnostic_messages_scroll_follow: bool,
    configuration_scroll_state: ScrollViewState,
    configuration_scroll_follow: bool,
    current_tab: SelectedTab,
    riot_board: String,
    riot_version: String,
}

impl UiState {
    fn new() -> Self {
        Self {
            device_path: None,

            current_tab: SelectedTab::Combined,

            configuration_scroll_state: ScrollViewState::default(),
            configuration_scroll_follow: true,
            diagnostic_messages_scroll_state: ScrollViewState::default(),
            diagnostic_messages_scroll_position: 0,
            diagnostic_messages_scroll_follow: true,

            riot_board: "Unkown".to_owned(),
            riot_version: "Unkown".to_owned(),
        }
    }

    fn set_board_name(&mut self, name: String) {
        self.riot_board = name;
    }

    fn set_board_version(&mut self, version: String) {
        self.riot_version = version;
    }

    fn get_config(&self) -> String {
        format!(
            "Version: {}\nBoard: {}\n",
            self.riot_version, self.riot_board,
        )
    }

    fn get_connection(&self) -> String {
        match &self.device_path {
            Some(device_path) => {
                format!(
                    "✅ connected via {device_path} with RIOT {}",
                    self.riot_version
                )
            }
            None => "❌ not connected, retrying..".to_owned(),
        }
    }

    fn scroll_down(&mut self) {
        self.diagnostic_messages_scroll_position =
            self.diagnostic_messages_scroll_position.saturating_sub(1);
        self.diagnostic_messages_scroll_follow = self.diagnostic_messages_scroll_position == 0;
        self.diagnostic_messages_scroll_state.scroll_down();

        // For the "sticky" behavior, where the view remains at the bottom, even if more
        // content is added
        if self.diagnostic_messages_scroll_follow {
            self.diagnostic_messages_scroll_state.scroll_to_bottom();
        }

        // For now, just have one global scrolling behavior
        self.configuration_scroll_follow = self.diagnostic_messages_scroll_follow;
        self.configuration_scroll_state.scroll_down();

        if self.configuration_scroll_follow {
            self.configuration_scroll_state.scroll_to_bottom();
        }
    }

    fn scroll_up(&mut self) {
        self.diagnostic_messages_scroll_follow = false;
        if self.diagnostic_messages_scroll_state.offset().y != 0 {
            self.diagnostic_messages_scroll_position =
                self.diagnostic_messages_scroll_position.saturating_add(1);
        }
        self.diagnostic_messages_scroll_state.scroll_up();

        self.configuration_scroll_follow = self.diagnostic_messages_scroll_follow;
        self.configuration_scroll_state.scroll_up();
    }
}

pub struct App {
    connected: bool,
    event_sender: Sender<Event>,
    configuration_log: Vec<Request>,
    configuration_packets: Vec<Packet>,
    diagnostic_log: DiagnosticLog,
    known_commands: CommandLibrary,
    user_input: String,
    user_command_history: Vec<String>,
    user_command_cursor: usize,
    token_count: u16,
    next_mid: u16,
    overall_log: DiagnosticLog,
    ongoing_jobs: HashMap<u64, usize>,
    ui_state: UiState,
    job_log: JobLog,
}

impl App {
    pub fn new(event_sender: Sender<Event>) -> Self {
        Self {
            connected: false,
            event_sender,

            configuration_log: vec![],
            configuration_packets: vec![],
            diagnostic_log: DiagnosticLog::new(),
            known_commands: CommandLibrary::default(),

            ui_state: UiState::new(),

            user_input: String::new(),
            user_command_history: vec![],
            user_command_cursor: 0,

            token_count: 0,
            next_mid: rand::rng().random(),

            overall_log: DiagnosticLog::new(),
            ongoing_jobs: HashMap::new(),
            job_log: JobLog::new(),
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
