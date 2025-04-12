use std::fmt::Write;

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
use ratatui::text::Line;
use ratatui::text::Span;
use slipmux::encode_configuration;

use crate::app::App;
use crate::app::Command;
use crate::events::Event;

impl App<'_> {
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
                    self.send_configuration_request(&request.message);
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

    pub fn on_connect(&mut self, name: String) {
        self.write_port = Some(name);

        let request: CoapRequest<String> = self.build_request("/riot/board");
        self.send_configuration_request(&request.message);
        self.configuration_requests.push(request);

        let request: CoapRequest<String> = self.build_request("/riot/ver");
        self.send_configuration_request(&request.message);
        self.configuration_requests.push(request);

        let request: CoapRequest<String> = self.build_request("/.well-known/core");
        self.send_configuration_request(&request.message);
        self.configuration_requests.push(request);
    }

    pub fn on_disconnect(&mut self) {
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
                '\t' | '\r' => (),
                _ => self
                    .diagnostic_messages
                    .push_span(Span::from(chr.to_string())),
            }
        }
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.diagnostic_messages_scroll_position =
                    self.diagnostic_messages_scroll_position.saturating_sub(1);
                self.diagnostic_messages_scroll_follow =
                    self.diagnostic_messages_scroll_position == 0;
                self.diagnostic_messages_scroll_state.scroll_down();

                // For now, just have one global scrolling behavior
                self.configuration_scroll_follow = self.diagnostic_messages_scroll_follow;
                self.configuration_scroll_state.scroll_down();
            }
            MouseEventKind::ScrollUp => {
                self.diagnostic_messages_scroll_follow = false;
                if self.diagnostic_messages_scroll_state.offset().y != 0 {
                    self.diagnostic_messages_scroll_position =
                        self.diagnostic_messages_scroll_position.saturating_add(1);
                }
                self.diagnostic_messages_scroll_state.scroll_up();

                self.configuration_scroll_follow = self.diagnostic_messages_scroll_follow;
                self.configuration_scroll_state.scroll_up();
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
                        if self.user_command != "/" {
                            // Might also be a bug in coap-lite that "/" should be turned into an
                            // empty option set; documentation isn't quite conclusive.
                            request.set_path(&self.user_command);
                        }
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
