use std::fmt::Write;
use std::io::Error;

use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::CoapResponse;
use coap_lite::Packet;
use coap_lite::RequestType as Method;

use crate::slipmux::{send_configuration, send_diagnostic};
use serialport::SerialPort;

mod tui3;

struct Command {
    cmd: String,
    description: String,
    location: Option<String>,
}
impl Command {
    pub fn new(cmd: String, description: String) -> Command {
        self {
            cmd,
            description,
            None,
        }
    }

    pub fn from_location(location: String, description: String) -> Command {
        let cmd: &str = location
            .strip_prefix("/shell/")
            .expect("Failed to parse shell command location!");
        self {
            cmd.to_string(),
            description,
            location,
        }
    }
}

pub struct App<'a> {
    write_port: Option<Box<dyn SerialPort>>,
    configuration_requests: Vec<CoapRequest<String>>,
    configuration_packets: Vec<Packet>,
    diagnostic_messages: Text<'a>,
    known_user_commands: Vec<(String, String)>,
    user_command: String,
    user_command_history: Vec<String>,
    user_command_cursor: usize,
    token_count: u16,
    riot_board: String,
    riot_version: String,
}
impl App<'_> {
    pub fn new() -> Self {
        Self {
            write_port: None,
            configuration_requests: vec![],
            configuration_packets: vec![],
            diagnostic_messages: Text::default(),
            known_user_commands: vec![
                (
                    "help".to_string(),
                    "Prints all available commands".to_string(),
                ),
                ("/.well-known/core".to_string(), "".to_string()),
            ],
            user_command: String::new(),
            user_command_history: vec![],
            user_command_cursor: 0,
            token_count: 0,
            riot_board: "Unkown".to_string(),
            riot_version: "Unkown".to_string(),
        }
    }

    fn get_new_token(&mut self) -> Vec<u8> {
        self.token_count += 1;
        self.token_count.to_le_bytes().to_vec()
    }

    fn build_request(&mut self, path: &str) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(path);
        request.message.set_token(self.get_new_token());
        request.message.add_option(CoapOption::Block2, vec![0x05]);
        request
    }

    fn send_request(&mut self, msg: &Packet) -> Result<(), Error> {
        let (data, size) = send_configuration(msg);
        if let Some(ref mut port) = &mut self.write_port {
            port.write_all(&data[..size])?;
            let _ = port.flush();
        }
        Ok(())
    }

    fn suggest_command(&self) -> Option<usize> {
        for (index, known_cmd) in self.known_user_commands.iter().enumerate() {
            if known_cmd.0.starts_with(&self.user_command) {
                return Some(index);
            };
        }
        None
    }

    fn on_well_known_core(&mut self, response: &Packet) {
        // Poor mans clif parser
        for s in String::from_utf8_lossy(&response.payload).split(',') {
            let tmp = s.to_string();
            let maybe = tmp.strip_prefix('<');
            if maybe.is_none() {
                continue;
            }
            let s = maybe.unwrap().split('>').next().unwrap().to_string();
            if !self
                .known_user_commands
                .contains(&(s.clone(), "".to_string()))
                && s.starts_with('/')
            {
                self.known_user_commands.push((s.clone(), "".to_string()));
                if s.starts_with("/shell/") {
                    let request: CoapRequest<String> = self.build_request(&s);
                    if let Err(_) = self.send_request(&request.message) {
                        self.diagnostic_messages
                            .push_line(Line::from("Failed to request /.well-known/core\n"));
                    } else {
                        self.configuration_requests.push(request);
                    }
                }
            }
        }
    }

    pub fn connect(&mut self, write_port: Box<dyn SerialPort>) {
        self.write_port = Some(write_port);

        let request: CoapRequest<String> = self.build_request("/riot/board");
        if let Err(_) = self.send_request(&request.message) {
            self.diagnostic_messages
                .push_line(Line::from("Failed to request /riot/board\n"));
        } else {
            self.configuration_requests.push(request);
        }

        // TODO: Fix me
        //thread::sleep(time::Duration::from_millis(10));

        let request: CoapRequest<String> = self.build_request("/riot/ver");
        if let Err(_) = self.send_request(&request.message) {
            self.diagnostic_messages
                .push_line(Line::from("Failed to request /riot/ver\n"));
        } else {
            self.configuration_requests.push(request);
        }

        // TODO: Fix me
        //thread::sleep(time::Duration::from_millis(20));

        let request: CoapRequest<String> = self.build_request("/.well-known/core");
        if let Err(_) = self.send_request(&request.message) {
            self.diagnostic_messages
                .push_line(Line::from("Failed to request /.well-known/core\n"));
        } else {
            self.configuration_requests.push(request);
        }
    }

    pub fn disconnect(&mut self) {
        self.write_port = None;
    }

    pub fn on_configuration_msg(&mut self, data: Vec<u8>) {
        let response = Packet::from_bytes(&data).unwrap();
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
                    _ = write!(uri_path, "/{}", String::from_utf8_lossy(option))
                }
                match uri_path.as_str() {
                    "/riot/board" => {
                        self.riot_board = String::from_utf8_lossy(&response.payload).to_string()
                    }
                    "/riot/ver" => {
                        self.riot_version = String::from_utf8_lossy(&response.payload).to_string()
                    }
                    "/.well-known/core" => {
                        self.on_well_known_core(&response);
                    }
                    _ => {
                        if uri_path.starts_with("/shell/") {
                            let dscr = String::from_utf8_lossy(&response.payload).to_string();
                            let maybeindex = self
                                .known_user_commands
                                .clone()
                                .into_iter()
                                .enumerate()
                                .find(|(_, (name, _))| uri_path.contains(name));
                            if let Some((index, _)) = maybeindex {
                                self.known_user_commands[index].1.push_str(&dscr);
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

    pub fn on_diagnostic_msg(&mut self, msg: String) {
        for chr in msg.chars() {
            match chr {
                '\n' => self.diagnostic_messages.push_line(Line::default()),
                _ => self
                    .diagnostic_messages
                    .push_span(Span::from(chr.to_string())),
            }
        }
        // self.diagnostic_messages.push_str(&format!("[{}]", &msg));
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }

        match key.code {
            KeyCode::Esc => return false,
            KeyCode::Enter => {
                if self.user_command.is_empty() {
                    return true;
                }
                if self.write_port.is_some() {
                    if self.user_command.starts_with('/') {
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Get);
                        request.set_path(&self.user_command);
                        request.message.set_token(self.get_new_token());
                        request.message.add_option(CoapOption::Block2, vec![0x05]);
                        let (data, size) = send_configuration(&request.message);
                        let _ = self.write_port.as_mut().unwrap().write(&data[..size]);
                        self.configuration_requests.push(request);
                    } else {
                        if !self.user_command.ends_with('\n') {
                            self.user_command.push('\n');
                        }
                        let (data, size) = send_diagnostic(&self.user_command);
                        let _ = self.write_port.as_mut().unwrap().write(&data[..size]);
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
                        .push_str(&self.known_user_commands[suggestion].0);
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
        };
        true
    }
}
