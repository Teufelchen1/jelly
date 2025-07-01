use std::fmt::Write;
use std::fs::File;
use std::io::Write as FileWrite;

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
use slipmux::encode_buffered;
use slipmux::Slipmux;

use super::SelectedTab;
use crate::app::App;
use crate::app::Job;
use crate::app::SaveToFile;
use crate::commands::Command;
use crate::events::Event;

fn hexdump(bin_data: &[u8]) -> String {
    let mut buffer = String::new();
    writeln!(buffer, "\n   |0 1 2 3  4 5 6 7  8 9 A B  C D E F").unwrap();
    for (index, chunk) in bin_data.chunks(16).enumerate() {
        write!(buffer, "{:03X}|", index * 16).unwrap();
        for minichunk in chunk.chunks(4) {
            for byte in minichunk {
                write!(buffer, "{byte:02X}").unwrap();
            }
            write!(buffer, " ").unwrap();
        }
        writeln!(buffer).unwrap();
    }
    buffer
}

impl App<'_> {
    fn on_well_known_core(&mut self, response: &Packet) {
        let mut eps: Vec<String> = vec![];
        // Poor mans clif parser
        for s in String::from_utf8_lossy(&response.payload).split(',') {
            let maybe = s.strip_prefix('<');
            if maybe.is_none() {
                continue;
            }
            let s = maybe.unwrap().split('>').next().unwrap();
            if s.starts_with('/') {
                eps.push(s.to_owned());
                // Skip commands that we already learned.
                if self.known_commands.find_by_first_location(s).is_some() {
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
                    let mut request: CoapRequest<String> = App::<'_>::build_get_request(s);
                    self.send_configuration_request(&mut request.message);
                    self.configuration_requests.push(request);
                } else {
                    let new_command = Command::from_coap_resource(s, "A CoAP resource");
                    self.known_commands.add(new_command);
                }
            }
        }
        self.known_commands
            .update_available_cmds_based_on_endpoints(&eps);
    }

    pub fn on_connect(&mut self, name: String) {
        self.write_port = Some(name);

        let mut request: CoapRequest<String> = App::<'_>::build_get_request("/riot/board");
        self.send_configuration_request(&mut request.message);
        self.configuration_requests.push(request);

        let mut request: CoapRequest<String> = App::<'_>::build_get_request("/riot/ver");
        self.send_configuration_request(&mut request.message);
        self.configuration_requests.push(request);

        let mut request: CoapRequest<String> = App::<'_>::build_get_request("/.well-known/core");
        self.send_configuration_request(&mut request.message);
        self.configuration_requests.push(request);
    }

    pub fn on_disconnect(&mut self) {
        self.write_port = None;
    }

    pub fn on_configuration_msg(&mut self, data: &[u8]) {
        let response = Packet::from_bytes(data).unwrap();

        // Get the key for the hashmap
        let token = response.get_token();
        let mut hash_index: u64 = 0;
        for byte in token {
            hash_index += u64::from(*byte);
        }

        // Do we have a job / handler for this request?
        // Removes it from the job list
        if let Some(mut job) = self.jobs.remove(&hash_index) {
            let mut buffer = String::new();
            let maybe_request = job.handler.handle(&response.payload);
            if job.handler.want_display() {
                match job.file {
                    SaveToFile::No => {
                        job.handler.display(&mut buffer);
                    }
                    SaveToFile::AsBin(ref file) => {
                        let bin_data: Vec<u8> = job.handler.export();
                        self.on_diagnostic_msg(&hexdump(&bin_data));
                        match File::create(file) {
                            Ok(mut f) => {
                                f.write_all(&bin_data).unwrap();
                                self.on_diagnostic_msg(&format!("(binary saved to: {file})\n"));
                            }
                            Err(e) => {
                                self.on_diagnostic_msg(&format!("(unable to write to {file}: {e}"));
                            }
                        }
                    }
                    SaveToFile::AsText(ref file) => {
                        job.handler.display(&mut buffer);
                        match File::create(file) {
                            Ok(mut f) => {
                                f.write_all(buffer.as_bytes()).unwrap();
                                self.on_diagnostic_msg(&format!("(saved to: {file})\n"));
                            }
                            Err(e) => {
                                self.on_diagnostic_msg(&format!("(unable to write to {file}: {e}"));
                            }
                        }
                    }
                }
                self.on_diagnostic_msg(&buffer);
            }
            // If we issue a new request, the token will change.
            // The token is our key for the hashmap, so we need to recalculate
            if let Some(mut next_request) = maybe_request {
                self.send_configuration_request(&mut next_request.message);
                hash_index = 0;
                for byte in next_request.message.get_token() {
                    hash_index += u64::from(*byte);
                }
                self.configuration_requests.push(next_request);
            }
            // Not finished? Re-add it to the job list
            if !job.handler.is_finished() {
                // This might be the new or the old key, depending if we send a new request.
                self.jobs.insert(hash_index, job);
            }
        }

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
                            if let Some(cmd) =
                                self.known_commands.find_by_first_location_mut(&uri_path)
                            {
                                let dscr = String::from_utf8_lossy(&response.payload);
                                cmd.update_description(&dscr);
                            }
                            if let Some(cmd) = self
                                .known_commands
                                .find_by_cmd_mut(uri_path.strip_prefix("/shell/").unwrap())
                            {
                                let dscr = String::from_utf8_lossy(&response.payload);
                                cmd.update_description(&dscr);
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

    fn handle_command_commit(&mut self) {
        enum InputType<'a> {
            /// The user input something that is not known to Jelly but it
            /// starts with a `/` so it likely is a coap endpoint
            /// Treated as configuration message
            RawCoap,
            /// The user input something that is not known to Jelly
            /// Treated as diagnostic message
            RawCommand,
            /// This input is a known command with a coap endpoint and a handler
            /// Treated as configuration message
            JellyCoapCommand(&'a Command, String, SaveToFile),
            /// This input is a known command without a coap endpoint
            /// Treated as diagnostic message
            JellyCommand(&'a Command),
        }

        let classify_input = |input: &str| {
            let (cmd_string, file) = if let Some((cmd_string, path)) = input.split_once("%>") {
                (cmd_string, SaveToFile::AsBin(path.trim().to_owned()))
            } else if let Some((cmd_string, path)) = input.split_once('>') {
                (cmd_string, SaveToFile::AsText(path.trim().to_owned()))
            } else {
                (input, SaveToFile::No)
            };
            let maybe_cmd = self
                .known_commands
                .find_by_cmd(cmd_string.split(' ').next().unwrap());
            match maybe_cmd {
                Some(cmd) => {
                    if cmd.required_endpoints.is_empty() {
                        InputType::JellyCommand(cmd)
                    } else {
                        InputType::JellyCoapCommand(cmd, cmd_string.to_owned(), file)
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

        match classify_input(&self.user_input) {
            InputType::RawCoap => {
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                if self.user_input != "/" {
                    // Might also be a bug in coap-lite that "/" should be turned into an
                    // empty option set; documentation isn't quite conclusive.
                    request.set_path(&self.user_input);
                }
                request.message.set_token(self.get_new_token());
                request.message.add_option(CoapOption::Block2, vec![0x05]);
                let data =
                    encode_buffered(Slipmux::Configuration(request.message.to_bytes().unwrap()));
                self.event_sender
                    .send(Event::SendConfiguration(data))
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
            InputType::JellyCoapCommand(cmd, cmd_string, file) => {
                // Process the user input string into arguments, yielding a handler
                let res = (cmd.parse)(cmd, cmd_string.clone());
                match res {
                    // User input matches the cli, done with argument parsing
                    Ok(mut handler) => {
                        let mut request = handler.init();
                        self.send_configuration_request(&mut request.message);
                        let mut hash_index: u64 = 0;
                        for byte in request.message.get_token() {
                            hash_index += u64::from(*byte);
                        }
                        self.configuration_requests.push(request.clone());
                        self.jobs.insert(hash_index, Job { handler, file });

                        // Mimiking RIOTs shell behavior for UX
                        self.on_diagnostic_msg(&cmd_string);
                        self.on_diagnostic_msg("\n> ");
                    }
                    // Display usage info to the user
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
            .push(self.user_input.clone().trim_end().to_owned());
        self.user_command_cursor = self.user_command_history.len();
        self.user_input.clear();
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }

        match key.code {
            KeyCode::Enter => {
                if self.write_port.is_none() {
                    return true;
                }
                self.handle_command_commit();
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
