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

use super::SelectedTab;
use crate::app::App;
use crate::commands::Command;
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
                // Skip commands that we already learned.
                if self.known_commands.find_by_location(s).is_some() {
                    continue;
                }
                if s.starts_with("/shell/") {
                    let new_command = Command::from_location(s, "A RIOT shell command");
                    self.known_commands.add(new_command);

                    let new_endpoint = Command::from_coap_resource(
                        s,
                        "A CoAP resource describing a RIOT shell command",
                    );
                    self.known_commands.add(new_endpoint);

                    // Fetch description
                    let mut request: CoapRequest<String> = self.build_get_request(s);
                    self.send_configuration_request(&mut request.message);
                    self.configuration_requests.push(request);
                } else {
                    let new_command = Command::from_coap_resource(s, "A CoAP resource");
                    self.known_commands.add(new_command);
                }
            }
        }
    }

    pub fn on_connect(&mut self, name: String) {
        self.write_port = Some(name);

        let mut request: CoapRequest<String> = self.build_get_request("/riot/board");
        self.send_configuration_request(&mut request.message);
        self.configuration_requests.push(request);

        let mut request: CoapRequest<String> = self.build_get_request("/riot/ver");
        self.send_configuration_request(&mut request.message);
        self.configuration_requests.push(request);

        let mut request: CoapRequest<String> = self.build_get_request("/.well-known/core");
        self.send_configuration_request(&mut request.message);
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
                        // RIOT specific hook
                        if uri_path.starts_with("/shell/") {
                            // If we already know this command, update it's description
                            if let Some(cmd) = self.known_commands.find_by_location_mut(&uri_path) {
                                let dscr = String::from_utf8_lossy(&response.payload);
                                cmd.update_description(&dscr);
                            }
                            if let Some(cmd) = self
                                .known_commands
                                .find_by_cmd_mut(&uri_path.strip_prefix("/shell/").unwrap())
                            {
                                let dscr = String::from_utf8_lossy(&response.payload);
                                cmd.update_description(&dscr);
                            }
                        }
                    }
                }
                match self
                    .known_commands
                    .find_by_location(uri_path.as_str())
                    .and_then(|cmd| cmd.display)
                {
                    Some(disp) => {
                        let dis = disp(response.payload);
                        self.on_diagnostic_msg(&dis);
                    }
                    None => (),
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
        enum InputType<'a> {
            /// The user input something that is not known to Jelly but it
            /// starts with a `/` so it likely is a coap endpoint
            /// Treated as configuration message
            RawCoap,
            /// The user input something that is not known to Jelly
            /// Treated as diagnostic message
            RawCommand,
            /// This input is a known command with a coap endpoint
            /// Treated as configuration message
            JellyCoap(()),
            /// This input is a known command with a coap endpoint and a handler
            /// Treated as configuration message
            JellyCoapCommand(&'a Command),
            /// This input is a known command without a coap endpoint
            /// Treated as diagnostic message
            JellyCommand(&'a Command),
        }
        let classify_input = |input: &str| {
            let maybe_cmd = self
                .known_commands
                .find_by_cmd(&input.split(' ').next().unwrap());
            match maybe_cmd {
                Some(cmd) => {
                    if cmd.location.is_some() {
                        if cmd.handler.is_some() {
                            InputType::JellyCoapCommand(cmd)
                        } else {
                            InputType::JellyCoap(() /* cmd */)
                        }
                    } else {
                        InputType::JellyCommand(cmd)
                    }
                }
                None => {
                    if input.starts_with('/') {
                        InputType::RawCoap
                    } else {
                        InputType::RawCommand
                    }
                }
            }
        };

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }

        match key.code {
            KeyCode::Enter => {
                if self.write_port.is_none() {
                    return true;
                }

                match classify_input(&self.user_input) {
                    InputType::RawCoap | InputType::JellyCoap(_) => {
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Get);
                        if self.user_input != "/" {
                            // Might also be a bug in coap-lite that "/" should be turned into an
                            // empty option set; documentation isn't quite conclusive.
                            request.set_path(&self.user_input);
                        }
                        request.message.set_token(self.get_new_token());
                        request.message.add_option(CoapOption::Block2, vec![0x05]);
                        let (data, size) =
                            encode_configuration(request.message.to_bytes().unwrap());
                        self.event_sender
                            .send(Event::SendConfiguration(data[..size].to_vec()))
                            .unwrap();
                        self.configuration_requests.push(request);
                    }
                    InputType::RawCommand => {
                        if !self.user_input.ends_with('\n') {
                            self.user_input.push('\n');
                        }
                        self.event_sender
                            .send(Event::SendDiagnostic(self.user_input.clone()))
                            .unwrap();
                    }
                    InputType::JellyCoapCommand(cmd) => {
                        let handler = cmd.handler.unwrap();
                        let res = handler(self.user_input.clone(), cmd.location.as_ref().unwrap());
                        match res {
                            Ok(mut req) => {
                                self.send_configuration_request(&mut req.message);
                                self.configuration_requests.push(req);
                            }
                            Err(e) => {
                                self.on_diagnostic_msg(&e);
                            }
                        }
                    }
                    InputType::JellyCommand(_cmd) => {
                        if !self.user_input.ends_with('\n') {
                            self.user_input.push('\n');
                        }
                        self.event_sender
                            .send(Event::SendDiagnostic(self.user_input.clone()))
                            .unwrap();
                    }
                }

                self.user_command_history
                    .push(self.user_input.clone().trim_end().to_string());
                self.user_command_cursor = self.user_command_history.len();
                self.user_input.clear();
            }
            KeyCode::Tab | KeyCode::Right => {
                let (suggestion, _) = self
                    .known_commands
                    .longest_common_prefixed_by_cmd(&self.user_input);

                self.user_input.clear();
                self.user_input.push_str(&suggestion);
            }
            KeyCode::Backspace => {
                self.user_input.pop();
            }
            KeyCode::Left => {}
            KeyCode::Up => {
                if self.user_command_cursor > 0 {
                    self.user_input.clear();
                    self.user_command_cursor -= 1;
                    self.user_input = self.user_command_history[self.user_command_cursor].clone();
                }
            }
            KeyCode::Down => {
                if self.user_command_cursor < self.user_command_history.len() {
                    self.user_input.clear();
                    self.user_command_cursor += 1;
                    if self.user_command_cursor == self.user_command_history.len() {
                        self.user_input.clear();
                    } else {
                        self.user_input =
                            self.user_command_history[self.user_command_cursor].clone();
                    }
                }
            }
            KeyCode::Char(to_insert) => {
                self.user_input.push(to_insert);
            }
            KeyCode::F(1) => {
                self.current_tab = SelectedTab::Combined;
            }
            KeyCode::F(2) => {
                self.current_tab = SelectedTab::Diagnostic;
            }
            KeyCode::F(3) => {
                self.current_tab = SelectedTab::Configuration;
            }
            _ => return false,
        }
        true
    }
}
