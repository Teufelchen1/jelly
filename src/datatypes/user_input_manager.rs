pub struct UserInputManager {
    pub user_input: String,
    user_command_history: Vec<String>,
    user_command_history_index: usize,
    pub cursor_position: usize,
}

impl UserInputManager {
    pub const fn new() -> Self {
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
    fn longest_common_prefix(prefix: &str, cmds: &[&String]) -> String {
        // Ideally we would use a tree here and also cache results
        match cmds.len() {
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
        }
    }

    pub fn suggestion<'a>(&'a self, command_library: &'a [&String]) -> (String, Vec<&'a String>) {
        let prefix = &self.user_input;

        let matching_from_history = self
            .user_command_history
            .iter()
            .filter(|cmd| cmd.starts_with(prefix));

        let matching_from_library = command_library
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .copied();
        let mut matching_both: Vec<&String> =
            matching_from_history.chain(matching_from_library).collect();
        matching_both.sort();
        matching_both.dedup();
        matching_both.retain(|cmd| **cmd != *prefix);

        let common_prefix = Self::longest_common_prefix(prefix, &matching_both);

        (common_prefix, matching_both)
    }

    pub fn set_suggest_completion(&mut self, command_library: &[&String]) {
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
}
