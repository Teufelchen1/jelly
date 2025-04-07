use std::sync::mpsc::Sender;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::Packet;
use coap_lite::RequestType as Method;
use rand::Rng;
use ratatui::text::Text;
use slipmux::encode_configuration;
use tui_widgets::scrollview::ScrollViewState;

use crate::app::commands::Command;
use crate::events::Event;

mod commands;
mod handler;
mod tui;

pub struct App<'a> {
    event_sender: Sender<Event>,
    write_port: Option<String>,
    configuration_requests: Vec<CoapRequest<String>>,
    configuration_packets: Vec<Packet>,
    configuration_scroll_state: ScrollViewState,
    configuration_scroll_follow: bool,
    pub diagnostic_messages: Text<'a>,
    diagnostic_messages_scroll_state: ScrollViewState,
    diagnostic_messages_scroll_position: usize,
    diagnostic_messages_scroll_follow: bool,
    known_user_commands: Vec<Command>,
    user_command: String,
    user_command_history: Vec<String>,
    user_command_cursor: usize,
    token_count: u16,
    riot_board: String,
    riot_version: String,
    next_mid: u16,
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
            known_user_commands: vec![
                Command::new("help", "Prints all available commands"),
                Command::new_coap_resource("/.well-known/core", "Query the wkc"),
            ],
            user_command: String::new(),
            user_command_history: vec![],
            user_command_cursor: 0,
            token_count: 0,
            riot_board: "Unkown".to_owned(),
            riot_version: "Unkown".to_owned(),

            next_mid: rand::rng().random(),
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

    fn build_request(&mut self, path: &str) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(path);
        request.message.header.message_id = self.get_new_message_id();
        request.message.set_token(self.get_new_token());
        request.message.add_option(CoapOption::Block2, vec![0x05]);
        request
    }

    fn send_configuration_request(&self, msg: &Packet) {
        let (data, size) = encode_configuration(msg.to_bytes().unwrap());
        self.event_sender
            .send(Event::SendConfiguration(data[..size].to_vec()))
            .unwrap();
    }

    fn suggest_command(&self) -> Option<usize> {
        for (index, known_cmd) in self.known_user_commands.iter().enumerate() {
            if known_cmd.cmd.starts_with(&self.user_command) {
                return Some(index);
            }
        }
        None
    }
}
