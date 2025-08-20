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
use crate::app::datatypes::SaveToFile;
use crate::commands::Command;
use crate::commands::CommandLibrary;
use crate::events::Event;

mod datatypes;
mod handler;
mod tui;

#[derive(Default, Clone, Copy)]
enum SelectedTab {
    #[default]
    Overview,
    Diagnostic,
    Configuration,
    Commands,
    Help,
}

struct ScrollState {
    state: ScrollViewState,
    position: usize,
    follow: bool,
}

impl ScrollState {
    fn new() -> Self {
        Self {
            state: ScrollViewState::default(),
            position: 0,
            follow: true,
        }
    }

    fn scroll_down(&mut self) {
        self.position = self.position.saturating_sub(1);
        // When scrolled all the way to the bottom, auto follow the feed ("sticky behavior")
        self.follow = self.position == 0;
        self.state.scroll_down();
    }

    fn scroll_up(&mut self) {
        self.follow = false;
        // Can't scroll up when already on top
        if self.state.offset().y != 0 {
            self.position = self.position.saturating_add(1);
        }
        self.state.scroll_up();
    }

    fn get_state_for_rendering(&mut self) -> &mut ScrollViewState {
        // For the "sticky" behavior, where the view remains at the bottom
        // Needs to be done during rendering as more content could have been added, making
        // a jump to the bottom necessary
        if self.follow {
            self.state.scroll_to_bottom();
        }

        &mut self.state
    }
}

struct UiState {
    device_path: Option<String>,
    overview_scroll: ScrollState,
    diagnostic_scroll: ScrollState,
    configuration_scroll: ScrollState,
    command_scroll: ScrollState,
    help_scroll: ScrollState,
    current_tab: SelectedTab,
    riot_board: String,
    riot_version: String,
}

impl UiState {
    fn new() -> Self {
        Self {
            device_path: None,

            current_tab: SelectedTab::Overview,

            overview_scroll: ScrollState::new(),
            diagnostic_scroll: ScrollState::new(),
            configuration_scroll: ScrollState::new(),
            command_scroll: ScrollState::new(),
            help_scroll: ScrollState::new(),

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
        match self.current_tab {
            SelectedTab::Overview => {
                self.overview_scroll.scroll_down();
                self.configuration_scroll.scroll_down();
            }
            SelectedTab::Diagnostic => self.diagnostic_scroll.scroll_down(),
            SelectedTab::Configuration => self.configuration_scroll.scroll_down(),
            SelectedTab::Commands => self.command_scroll.scroll_down(),
            SelectedTab::Help => self.help_scroll.scroll_down(),
        }
    }

    fn scroll_up(&mut self) {
        match self.current_tab {
            SelectedTab::Overview => {
                self.overview_scroll.scroll_up();
                self.configuration_scroll.scroll_up();
            }
            SelectedTab::Diagnostic => self.diagnostic_scroll.scroll_up(),
            SelectedTab::Configuration => self.configuration_scroll.scroll_up(),
            SelectedTab::Commands => self.command_scroll.scroll_up(),
            SelectedTab::Help => self.help_scroll.scroll_up(),
        }
    }
}

struct UserInputManager {
    known_commands: CommandLibrary,
    user_input: String,
    user_command_history: Vec<String>,
    user_command_history_index: usize,
    cursor_position: usize,
}

enum InputType<'a> {
    /// The user input something that is not known to Jelly but it
    /// starts with a `/` so it likely is a coap endpoint
    /// Treated as configuration message
    RawCoap(String),
    /// The user input something that is not known to Jelly
    /// Treated as diagnostic message
    RawCommand(String),
    /// This input is a known command with a coap endpoint and a handler
    /// Treated as configuration message
    JellyCoapCommand(&'a Command, String, SaveToFile),
    /// This input is a known command without a coap endpoint
    /// Treated as diagnostic message
    JellyCommand(&'a Command),
}

impl UserInputManager {
    fn new() -> Self {
        Self {
            known_commands: CommandLibrary::default(),
            user_input: String::new(),
            user_command_history: vec![],
            user_command_history_index: 0,
            cursor_position: 0,
        }
    }

    fn insert_char(&mut self, chr: char) {
        self.user_input.insert(self.cursor_position, chr);
        self.cursor_position += 1;
    }

    fn remove_char(&mut self) {
        if self.cursor_position > 0 && self.cursor_position <= self.user_input.len() {
            self.cursor_position = self.cursor_position.saturating_sub(1);
            self.user_input.remove(self.cursor_position);
        }
    }

    const fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    const fn move_cursor_right(&mut self) {
        if self.cursor_position < self.user_input.len() {
            self.cursor_position += 1;
        }
    }

    fn suggestion(&self) -> (String, Vec<&Command>) {
        self.known_commands
            .longest_common_prefixed_by_cmd(&self.user_input)
    }

    fn set_suggest_completion(&mut self) {
        let (suggestion, _) = self
            .known_commands
            .longest_common_prefixed_by_cmd(&self.user_input);

        self.user_input.clear();
        self.user_input.push_str(&suggestion);
        self.cursor_position = self.user_input.len();
    }

    fn set_to_previous_input(&mut self) {
        if self.user_command_history_index > 0 {
            self.user_command_history_index -= 1;
            self.user_input = self.user_command_history[self.user_command_history_index].clone();
            self.cursor_position = self.user_input.len();
        }
    }

    fn set_to_next_input(&mut self) {
        if self.user_command_history_index < self.user_command_history.len() {
            self.user_command_history_index += 1;
            if self.user_command_history_index == self.user_command_history.len() {
                self.user_input.clear();
                self.cursor_position = 0;
            } else {
                self.user_input =
                    self.user_command_history[self.user_command_history_index].clone();
                self.cursor_position = self.user_input.len();
            }
        }
    }

    fn finish_current_input(&mut self) {
        // We don't want to store empty inputs
        if !self.user_input.is_empty() {
            // nor the same command multiple times
            let last_command_equals_current = self
                .user_command_history
                .last()
                .is_some_and(|cmd| *cmd == self.user_input);
            if !last_command_equals_current {
                self.user_command_history
                    .push(self.user_input.clone().trim_end().to_owned());
            }
            self.user_input.clear();
            self.cursor_position = 0;
        }
        // This has to be done even if the input is empty, as the user might have scrolled back
        // and deleted all input.
        self.user_command_history_index = self.user_command_history.len();
    }

    const fn input_empty(&self) -> bool {
        self.user_input.is_empty()
    }

    fn classify_input(&self) -> InputType<'_> {
        let (cmd_string, file) = if let Some((cmd_string, path)) = self.user_input.split_once("%>")
        {
            (cmd_string, SaveToFile::AsBin(path.trim().to_owned()))
        } else if let Some((cmd_string, path)) = self.user_input.split_once('>') {
            (cmd_string, SaveToFile::AsText(path.trim().to_owned()))
        } else {
            (self.user_input.as_str(), SaveToFile::No)
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
                if self.user_input.starts_with('/') {
                    InputType::RawCoap(self.user_input.clone())
                } else {
                    let mut cmd = self.user_input.clone();
                    if !cmd.ends_with('\n') {
                        cmd.push('\n');
                    }
                    InputType::RawCommand(cmd)
                }
            }
        }
    }

    fn command_name_list(&self) -> String {
        self.known_commands.list_by_cmd().join(", ")
    }

    fn command_exists_by_location(&self, location: &str) -> bool {
        self.known_commands
            .find_by_first_location(location)
            .is_some()
    }

    fn check_for_new_available_commands(&mut self, eps: &[String]) {
        self.known_commands
            .update_available_cmds_based_on_endpoints(eps);
    }

    fn update_command_description_by_location(&mut self, location: &str, description: &str) {
        // If we already know this command, update it's description
        if let Some(cmd) = self.known_commands.find_by_first_location_mut(location) {
            cmd.update_description(description);
        }
    }

    fn update_command_description_by_name(&mut self, name: &str, description: &str) {
        // If we already know this command, update it's description
        if let Some(cmd) = self.known_commands.find_by_cmd_mut(name) {
            cmd.update_description(description);
        }
    }
}

pub struct App {
    connected: bool,
    event_sender: Sender<Event>,
    configuration_log: Vec<Request>,
    configuration_packets: Vec<Packet>,
    diagnostic_log: DiagnosticLog,
    user_input_manager: UserInputManager,
    ui_state: UiState,
    token_count: u16,
    next_mid: u16,
    overall_log: DiagnosticLog,
    ongoing_jobs: HashMap<u64, usize>,
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

            user_input_manager: UserInputManager::new(),

            ui_state: UiState::new(),

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
