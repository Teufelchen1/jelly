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

use crate::app::App;
use crate::app::InputType;
use crate::app::Job;
use crate::app::Request;
use crate::command::Command;
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
                if self.user_input_manager.command_exists_by_location(s) {
                    continue;
                }
                if s.starts_with("/shell/") {
                    let new_command = Command::from_location(s, "A RIOT shell command");
                    self.user_input_manager.known_commands.add(new_command);

                    let new_endpoint = Command::from_coap_resource(
                        s,
                        "A CoAP resource describing a RIOT shell command",
                    );
                    self.user_input_manager.known_commands.add(new_endpoint);

                    // Fetch description
                    let mut request: CoapRequest<String> = Self::build_get_request(s);
                    self.send_configuration_request(&mut request.message);
                    self.configuration_log.push(Request::new(request));
                } else {
                    let new_command = Command::from_coap_resource(s, "A CoAP resource");
                    self.user_input_manager.known_commands.add(new_command);
                }
            }
        }

        self.user_input_manager
            .check_for_new_available_commands(&eps);
    }

    pub fn on_connect(&mut self, name: String) {
        self.connected = true;
        self.ui_state.set_device_path(name);

        let mut request: CoapRequest<String> = Self::build_get_request("/.well-known/core");
        self.send_configuration_request(&mut request.message);
        self.configuration_log.push(Request::new(request));

        let mut request: CoapRequest<String> = Self::build_get_request("/jelly/board");
        self.send_configuration_request(&mut request.message);
        self.configuration_log.push(Request::new(request));

        let mut request: CoapRequest<String> = Self::build_get_request("/jelly/ver");
        self.send_configuration_request(&mut request.message);
        self.configuration_log.push(Request::new(request));
    }

    pub fn on_disconnect(&mut self) {
        self.connected = false;
        self.ui_state.clear_device_path();
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
                hash_index = 0;
                for byte in next_request.message.get_token() {
                    hash_index += u64::from(*byte);
                }
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
                    "/jelly/board" => self
                        .ui_state
                        .set_board_name(String::from_utf8_lossy(&response.payload).to_string()),
                    "/jelly/ver" => self
                        .ui_state
                        .set_board_version(String::from_utf8_lossy(&response.payload).to_string()),

                    "/.well-known/core" => self.on_well_known_core(&response),
                    _ => {
                        // RIOT specific hook
                        if uri_path.starts_with("/shell/") {
                            let dscr = String::from_utf8_lossy(&response.payload);
                            self.user_input_manager
                                .update_command_description_by_location(&uri_path, &dscr);

                            self.user_input_manager.update_command_description_by_name(
                                uri_path.strip_prefix("/shell/").unwrap(),
                                &dscr,
                            );
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
                self.ui_state.scroll_down();
            }
            MouseEventKind::ScrollUp => {
                self.ui_state.scroll_up();
            }
            _ => {}
        }
        true
    }

    fn handle_command_commit(&mut self) {
        match self.user_input_manager.classify_input() {
            InputType::RawCoap(endpoint) => {
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                if endpoint != "/" {
                    // Might also be a bug in coap-lite that "/" should be turned into an
                    // empty option set; documentation isn't quite conclusive.
                    request.set_path(&endpoint);
                }
                request.message.set_token(self.get_new_token());
                let data =
                    encode_buffered(Slipmux::Configuration(request.message.to_bytes().unwrap()));
                self.event_sender
                    .send(Event::SendConfiguration(data))
                    .unwrap();
                self.configuration_log.push(Request::new(request));
            }
            InputType::RawCommand(cmd) => {
                self.event_sender.send(Event::SendDiagnostic(cmd)).unwrap();
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
            InputType::JellyCommand(_cmd, mut cmd_str) => {
                if !cmd_str.ends_with('\n') {
                    cmd_str.push('\n');
                }
                self.event_sender
                    .send(Event::SendDiagnostic(cmd_str))
                    .unwrap();
            }
        }

        self.user_input_manager.finish_current_input();
    }

    pub fn on_msg_string(&mut self, msg: &str) {
        self.user_input_manager.insert_string(msg);

        // should always be the case as msg is read via read_line()
        if msg.ends_with('\n') {
            self.handle_command_commit();
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }

        match key.code {
            KeyCode::Enter => {
                // Can't send anything if we don't have an active connection
                if self.connected {
                    self.handle_command_commit();
                }
            }
            KeyCode::Tab => {
                self.user_input_manager.set_suggest_completion();
            }
            KeyCode::Backspace => {
                self.user_input_manager.remove_char();
            }
            KeyCode::Left => {
                self.user_input_manager.move_cursor_left();
            }
            KeyCode::Right => {
                self.user_input_manager.move_cursor_right();
            }
            KeyCode::Up => {
                self.user_input_manager.set_to_previous_input();
            }
            KeyCode::Down => {
                self.user_input_manager.set_to_next_input();
            }
            KeyCode::Char(to_insert) => {
                self.user_input_manager.insert_char(to_insert);
            }
            KeyCode::F(1) => {
                self.ui_state.select_overview_view();
            }
            KeyCode::F(2) => {
                self.ui_state.select_diagnostic_view();
            }
            KeyCode::F(3) => {
                self.ui_state.select_configuration_view();
            }
            KeyCode::F(4) => {
                self.ui_state.select_commands_view();
            }
            KeyCode::F(5) => {
                self.ui_state.select_help_view();
            }
            KeyCode::Esc => {
                return false;
            }
            _ => {}
        }
        true
    }
}
