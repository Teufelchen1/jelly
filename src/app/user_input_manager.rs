use crate::app::SaveToFile;
use crate::command::Command;
use crate::command::CommandLibrary;

pub struct UserInputManager {
    pub known_commands: CommandLibrary,
    pub user_input: String,
    user_command_history: Vec<String>,
    user_command_history_index: usize,
    pub cursor_position: usize,
}

pub enum InputType<'a> {
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
    JellyCommand(&'a Command, String),
}

impl UserInputManager {
    pub fn new() -> Self {
        Self {
            known_commands: CommandLibrary::default(),
            user_input: String::new(),
            user_command_history: vec![],
            user_command_history_index: 0,
            cursor_position: 0,
        }
    }

    pub fn force_all_commands_availabe(&mut self) {
        self.known_commands.force_all_cmds_available();
    }

    pub fn insert_string(&mut self, string: &str) {
        self.user_input.push_str(string);
        self.cursor_position += string.len();
    }

    pub fn insert_char(&mut self, chr: char) {
        self.user_input.insert(self.cursor_position, chr);
        self.cursor_position += 1;
    }

    pub fn remove_char(&mut self) {
        if self.cursor_position > 0 && self.cursor_position <= self.user_input.len() {
            self.cursor_position = self.cursor_position.saturating_sub(1);
            self.user_input.remove(self.cursor_position);
        }
    }

    pub const fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    pub const fn move_cursor_right(&mut self) {
        if self.cursor_position < self.user_input.len() {
            self.cursor_position += 1;
        }
    }

    pub fn suggestion(&self) -> (String, Vec<&Command>) {
        self.known_commands
            .longest_common_prefixed_by_cmd(&self.user_input)
    }

    pub fn set_suggest_completion(&mut self) {
        let (suggestion, _) = self
            .known_commands
            .longest_common_prefixed_by_cmd(&self.user_input);

        self.user_input.clear();
        self.user_input.push_str(&suggestion);
        self.cursor_position = self.user_input.len();
    }

    pub fn set_to_previous_input(&mut self) {
        if self.user_command_history_index > 0 {
            self.user_command_history_index -= 1;
            self.user_input = self.user_command_history[self.user_command_history_index].clone();
            self.cursor_position = self.user_input.len();
        }
    }

    pub fn set_to_next_input(&mut self) {
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

    pub fn finish_current_input(&mut self) {
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

    pub const fn input_empty(&self) -> bool {
        self.user_input.is_empty()
    }

    pub fn classify_input(&self) -> InputType<'_> {
        let (cmd_string, file) = if let Some((cmd_string, path)) = self.user_input.split_once("%>")
        {
            let path = path.trim();
            // To Stdout
            if path == "-" {
                (cmd_string, SaveToFile::ToStdout)
            } else {
                (cmd_string, SaveToFile::AsBin(path.to_owned()))
            }
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
                    InputType::JellyCommand(cmd, cmd_string.to_owned())
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

    pub fn command_name_list(&self) -> String {
        self.known_commands.list_by_cmd().join(", ")
    }

    pub fn command_exists_by_location(&self, location: &str) -> bool {
        self.known_commands
            .find_by_first_location(location)
            .is_some()
    }

    pub fn check_for_new_available_commands(&mut self, eps: &[String]) {
        self.known_commands
            .update_available_cmds_based_on_endpoints(eps);
    }

    pub fn update_command_description_by_location(&mut self, location: &str, description: &str) {
        // If we already know this command, update it's description
        if let Some(cmd) = self.known_commands.find_by_first_location_mut(location) {
            cmd.update_description(description);
        }
    }

    pub fn update_command_description_by_name(&mut self, name: &str, description: &str) {
        // If we already know this command, update it's description
        if let Some(cmd) = self.known_commands.find_by_cmd_mut(name) {
            cmd.update_description(description);
        }
    }
}
