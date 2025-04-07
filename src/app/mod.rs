use std::fmt::Write;
use std::sync::mpsc::Sender;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::CoapResponse;
use coap_lite::Packet;
use coap_lite::RequestType as Method;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use rand::Rng;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use slipmux::encode_configuration;
use tui_scrollview::ScrollViewState;

use crate::events::Event;

mod tui;

struct Command {
    pub cmd: String,
    pub description: String,
    pub _location: Option<String>,
}
impl Command {
    pub fn new(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            _location: None,
        }
    }

    pub fn new_coap_resource(resource: &str, description: &str) -> Self {
        Self {
            cmd: resource.to_owned(),
            description: description.to_owned(),
            _location: Some(resource.to_owned()),
        }
    }

    pub fn from_location(location: &str, description: &str) -> Self {
        let cmd = location
            .strip_prefix("/shell/")
            .expect("Failed to parse shell command location!");
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            _location: Some(location.to_owned()),
        }
    }
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.cmd == other.cmd
    }
}

pub struct App<'a> {
    event_sender: Sender<Event>,
    write_port: Option<String>,
    configuration_requests: Vec<CoapRequest<String>>,
    configuration_packets: Vec<Packet>,
    configuration_scroll_position: ScrollViewState,
    configuration_scroll_position_follow: bool,
    pub diagnostic_messages: Text<'a>,
    diagnostic_messages_scroll_position: usize,
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
            configuration_scroll_position: ScrollViewState::default(),
            configuration_scroll_position_follow: true,
            diagnostic_messages: Text::default(),
            diagnostic_messages_scroll_position: 0,
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

    fn send_request(&self, msg: &Packet) {
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

    fn on_well_known_core(&mut self, response: &Packet) {
        // Poor mans clif parser
        for s in String::from_utf8_lossy(&response.payload).split(',') {
            let maybe = s.strip_prefix('<');
            if maybe.is_none() {
                continue;
            }
            let s = maybe.unwrap().split('>').next().unwrap();
            if s.starts_with('/') {
                if s.starts_with("/shell/") {
                    let new_command = Command::from_location(s, "A CoAP resource");

                    // Skip commands that we already learned.
                    if self.known_user_commands.contains(&new_command) {
                        continue;
                    }
                    self.known_user_commands.push(new_command);
                    let request: CoapRequest<String> = self.build_request(s);
                    self.send_request(&request.message);
                    self.configuration_requests.push(request);
                } else {
                    let new_command = Command::new_coap_resource(s, "A CoAP resource");

                    // Skip commands that we already learned.
                    if self.known_user_commands.contains(&new_command) {
                        continue;
                    }
                    self.known_user_commands.push(new_command);
                }
            }
            // TODO: Fix me
            //thread::sleep(time::Duration::from_millis(10));
        }
    }

    pub fn connect(&mut self, name: String) {
        self.write_port = Some(name);

        let request: CoapRequest<String> = self.build_request("/riot/board");
        self.send_request(&request.message);
        self.configuration_requests.push(request);

        // TODO: Fix me
        //thread::sleep(time::Duration::from_millis(1000));

        let request: CoapRequest<String> = self.build_request("/riot/ver");
        self.send_request(&request.message);
        self.configuration_requests.push(request);

        // TODO: Fix me
        //thread::sleep(time::Duration::from_millis(2000));

        let request: CoapRequest<String> = self.build_request("/.well-known/core");
        self.send_request(&request.message);
        self.configuration_requests.push(request);
    }

    pub fn disconnect(&mut self) {
        self.write_port = None;
    }

    pub fn on_configuration_msg(&mut self, data: &[u8]) {
        let response = Packet::from_bytes(data).unwrap();
        let token = response.get_token();
        let found_matching_request = self
            .configuration_requests
            .iter_mut()
            .find(|req| req.message.get_token() == token);
        if let Some(request) = found_matching_request {
            request.response = Some(CoapResponse {
                message: response.clone(),
            });
            let option_list_ = request.message.get_option(CoapOption::UriPath);
            if let Some(option_list) = option_list_ {
                let mut uri_path = String::new();
                for option in option_list {
                    _ = write!(uri_path, "/{}", String::from_utf8_lossy(option));
                }
                match uri_path.as_str() {
                    "/riot/board" => {
                        self.riot_board = String::from_utf8_lossy(&response.payload).to_string();
                    }
                    "/riot/ver" => {
                        self.riot_version = String::from_utf8_lossy(&response.payload).to_string();
                    }
                    "/.well-known/core" => self.on_well_known_core(&response),
                    _ => {
                        if uri_path.starts_with("/shell/") {
                            let dscr = String::from_utf8_lossy(&response.payload);
                            let maybeindex = self
                                .known_user_commands
                                .iter()
                                .enumerate()
                                .find(|(_, cmd)| uri_path.contains(&cmd.cmd));
                            if let Some((index, _)) = maybeindex {
                                self.known_user_commands[index].description.clear();
                                self.known_user_commands[index].description.push_str(&dscr);
                            }
                        }
                    }
                }
            }
        } else {
            // This should never happen, as it means that the riot node
            // proactively send a configuration message
            self.configuration_packets.push(response);
        }
    }

    pub fn on_diagnostic_msg(&mut self, msg: &str) {
        for chr in msg.chars() {
            match chr {
                '\n' => self.diagnostic_messages.push_line(Line::default()),
                _ => self
                    .diagnostic_messages
                    .push_span(Span::from(chr.to_string())),
            }
        }
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.configuration_scroll_position_follow = false;
                self.configuration_scroll_position.scroll_down();
                self.diagnostic_messages_scroll_position =
                    self.diagnostic_messages_scroll_position.saturating_add(1);
            }
            MouseEventKind::ScrollUp => {
                self.configuration_scroll_position_follow = false;
                self.configuration_scroll_position.scroll_up();
                self.diagnostic_messages_scroll_position =
                    self.diagnostic_messages_scroll_position.saturating_sub(1);
            }

            _ => {}
        }
        true
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }

        match key.code {
            KeyCode::Enter => {
                if self.write_port.is_some() {
                    if self.user_command.starts_with('/') {
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Get);
                        request.set_path(&self.user_command);
                        request.message.set_token(self.get_new_token());
                        request.message.add_option(CoapOption::Block2, vec![0x05]);
                        let (data, size) =
                            encode_configuration(request.message.to_bytes().unwrap());
                        self.event_sender
                            .send(Event::SendConfiguration(data[..size].to_vec()))
                            .unwrap();
                        self.configuration_requests.push(request);
                    } else {
                        if !self.user_command.ends_with('\n') {
                            self.user_command.push('\n');
                        }
                        self.event_sender
                            .send(Event::SendDiagnostic(self.user_command.clone()))
                            .unwrap();
                    }
                    self.user_command_history.push(self.user_command.clone());
                    self.user_command_cursor = self.user_command_history.len();
                    self.user_command.clear();
                }
            }
            KeyCode::Tab | KeyCode::Right => {
                if let Some(suggestion) = self.suggest_command() {
                    self.user_command.clear();
                    self.user_command
                        .push_str(&self.known_user_commands[suggestion].cmd);
                }
            }
            KeyCode::Backspace => {
                self.user_command.pop();
            }
            KeyCode::Up => {
                if self.user_command_cursor > 0 {
                    self.user_command.clear();
                    self.user_command_cursor -= 1;
                    self.user_command = self.user_command_history[self.user_command_cursor].clone();
                }
            }
            KeyCode::Down => {
                if self.user_command_cursor < self.user_command_history.len() {
                    self.user_command.clear();
                    self.user_command_cursor += 1;
                    if self.user_command_cursor == self.user_command_history.len() {
                        self.user_command.clear();
                    } else {
                        self.user_command =
                            self.user_command_history[self.user_command_cursor].clone();
                    }
                }
            }
            KeyCode::Char(to_insert) => {
                self.user_command.push(to_insert);
            }
            _ => return false,
        }
        true
    }
}
