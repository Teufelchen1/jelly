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
use slipmux::encode_buffered;
use slipmux::Slipmux;

use super::datatypes::token_to_u64;
use super::datatypes::Response;
use super::datatypes::SaveToFile;
use super::SelectedTab;
use crate::app::App;
use crate::app::Job;
use crate::app::Request;
use crate::commands::Command;
use crate::events::Event;

impl App {
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
                    let mut request: CoapRequest<String> = Self::build_get_request(s);
                    self.send_configuration_request(&mut request.message);
                    self.configuration_log.push(Request::new(request));
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

        let mut request: CoapRequest<String> = Self::build_get_request("/riot/board");
        self.send_configuration_request(&mut request.message);
        self.configuration_log.push(Request::new(request));

        let mut request: CoapRequest<String> = Self::build_get_request("/riot/ver");
        self.send_configuration_request(&mut request.message);
        self.configuration_log.push(Request::new(request));

        let mut request: CoapRequest<String> = Self::build_get_request("/.well-known/core");
        self.send_configuration_request(&mut request.message);
        self.configuration_log.push(Request::new(request));
    }

    pub fn on_disconnect(&mut self) {
        self.write_port = None;
    }

    pub fn on_tick(&mut self) {
        let keys: Vec<u64> = self.ongoing_jobs.keys().map(|x| *x).collect();
        for mut hash_index in keys {
            // Removes it from the job list here
            if let Some(job_id) = self.ongoing_jobs.remove(&hash_index) {
                let maybe_request = self.job_log.job_tick(job_id);
                if self.job_log.job_wants_display(job_id) {
                    let buffer = self.job_log.job_display(job_id);
                    self.overall_log.add(&buffer);
                }
                // If we issue a new request, the token will change.
                // The token is our key for the hashmap, so we need to recalculate
                if let Some(mut next_request) = maybe_request {
                    self.send_configuration_request(&mut next_request.message);
                    hash_index = token_to_u64(next_request.message.get_token());

                    self.configuration_log.push(Request::new(next_request));
                }
                // Not finished? Re-add it to the job list
                if self.job_log.job_is_finished(job_id) {
                    self.job_log.job_finish(job_id);
                } else {
                    // This might be the new or the old key, depending if we send a new request.
                    self.ongoing_jobs.insert(hash_index, job_id);
                }
            }
        }
    }

    fn handle_pending_job(&mut self, mut hash_index: u64, payload: &[u8]) {
        // Do we have a job / handler for this request?
        // Removes it from the job list here
        if let Some(job_id) = self.ongoing_jobs.remove(&hash_index) {
            let maybe_request = self.job_log.job_handle_payload(job_id, payload);
            if self.job_log.job_wants_display(job_id) {
                let buffer = self.job_log.job_display(job_id);
                self.overall_log.add(&buffer);
            }
            // If we issue a new request, the token will change.
            // The token is our key for the hashmap, so we need to recalculate
            if let Some(mut next_request) = maybe_request {
                self.send_configuration_request(&mut next_request.message);
                hash_index = token_to_u64(next_request.message.get_token());
                self.configuration_log.push(Request::new(next_request));
            }
            // Not finished? Re-add it to the job list
            if self.job_log.job_is_finished(job_id) {
                self.job_log.job_finish(job_id);
            } else {
                // This might be the new or the old key, depending if we send a new request.
                self.ongoing_jobs.insert(hash_index, job_id);
            }
        }
    }

    pub fn on_configuration_msg(&mut self, data: &[u8]) {
        let response = Packet::from_bytes(data).unwrap();

        // Get the key for the hashmap
        let token = response.get_token();
        let hash_index = token_to_u64(token);

        // Do we have a job / handler for this request?
        // Removes it from the job list if finished
        self.handle_pending_job(hash_index, &response.payload);

        let found_matching_request = self
            .configuration_log
            .iter()
            .position(|req| req.token == hash_index);
        if let Some(request_pos) = found_matching_request {
            self.configuration_log[request_pos]
                .res
                .push(Response::new(CoapResponse {
                    message: response.clone(),
                }));

            let request = &self.configuration_log[request_pos].req;
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
        self.diagnostic_log.add(msg);
        self.overall_log.add(msg);
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
                self.configuration_log.push(Request::new(request));
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
                        let hash_index: u64 = token_to_u64(request.message.get_token());
                        self.configuration_log.push(Request::new(request));
                        let job_id =
                            self.job_log
                                .start(Job::new(handler, file, cmd_string.clone()));
                        self.ongoing_jobs.insert(hash_index, job_id);

                        // Mimiking RIOTs shell behavior for UX
                        self.overall_log.add(&cmd_string);
                        self.overall_log.add("\n> ");
                    }
                    // Display usage info to the user
                    Err(e) => {
                        self.job_log.start(Job::new_failed(cmd_string.clone(), &e));
                        self.overall_log.add(&e);
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
            KeyCode::F(4) => {
                self.current_tab = SelectedTab::Commands;
            }
            KeyCode::F(5) => {
                self.current_tab = SelectedTab::Help;
            }
            KeyCode::Esc => {
                return false;
            }
            _ => {}
        }
        true
    }
}
