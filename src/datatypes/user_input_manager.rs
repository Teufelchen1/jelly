use super::job_log::SaveToFile;

use crate::command::Command;
use crate::command::CommandLibrary;

pub struct UserInputManager {
    pub user_input: String,
    user_command_history: Vec<String>,
    user_command_history_index: usize,
    pub cursor_position: usize,
}

pub enum InputType {
    /// The user input something that is not known to Jelly but it
    /// starts with a `/` so it likely is a coap endpoint
    /// Treated as configuration message
    RawCoap(String),
    /// The user input something that is not known to Jelly
    /// Treated as diagnostic message
    RawCommand(String),
    /// This input is a known command
    Command(String, String, SaveToFile),
}

impl UserInputManager {
    pub fn new() -> Self {
        Self {
            user_input: String::new(),
            user_command_history: vec![],
            user_command_history_index: 0,
            cursor_position: 0,
        }
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

    /// Takes a prefix and a list of Strings, computes the longest common prefix.
    /// For example if the given prefix `F` matches `FooBar`, `FooBaz` and `FooBizz`, this
    /// function would return `FooB`.
    /// On empty input, prefix is returened as is
    fn longest_common_prefix(&self, prefix: &str, cmds: &[&String]) -> String {
        // Ideally we would use a tree here and also cache results
        let actual_prefix = match cmds.len() {
            0 => prefix.to_owned(),
            1 => cmds[0].clone(),
            _ => {
                let mut common_prefix = prefix.to_owned();
                let first_cmd = &cmds[0];
                'outer: for (i, character) in first_cmd.chars().enumerate().skip(prefix.len()) {
                    for othercmd in cmds.iter().skip(1) {
                        if i >= othercmd.len() || othercmd.chars().nth(i) != Some(character) {
                            break 'outer;
                        }
                    }
                    common_prefix.push(character);
                }
                common_prefix
            }
        };
        actual_prefix
    }

    pub fn suggestion<'a>(
        &'a self,
        command_library: &'a CommandLibrary,
    ) -> (String, Vec<&'a String>) {
        let prefix = &self.user_input;

        let matching_from_history = self
            .user_command_history
            .iter()
            .filter(|cmd| cmd.starts_with(prefix));

        let matching_from_library = command_library.matching_prefix_by_cmd(prefix);
        let mut matching_both: Vec<&String> = matching_from_history
            .chain(matching_from_library.iter().map(|c| &c.cmd))
            .collect();
        matching_both.sort();
        matching_both.dedup();
        matching_both.retain(|cmd| **cmd != *prefix);

        let common_prefix = self.longest_common_prefix(prefix, &matching_both);

        (common_prefix, matching_both)
    }

    pub fn set_suggest_completion(&mut self, command_library: &CommandLibrary) {
        let (suggestion, _) = self.suggestion(command_library);

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

    pub fn classify_input(&self, command_library: &CommandLibrary) -> InputType {
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
        let maybe_cmd = command_library.find_by_cmd(cmd_string.split(' ').next().unwrap());
        match maybe_cmd {
            Some(cmd) => InputType::Command(cmd.cmd.clone(), cmd_string.to_owned(), file),
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
}
