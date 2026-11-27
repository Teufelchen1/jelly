pub struct UserInputManager {
    pub user_input: String,
    user_command_history: Vec<String>,
    user_command_history_index: usize,
    // Cursor position is exposed to calculated linewrap in the TUI
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_indeed_empty() {
        let mut manager = UserInputManager::new();
        assert!(manager.input_empty());

        manager.insert_string("Hello World!");
        assert!(!manager.input_empty());
    }

    #[test]
    fn can_insert_or_type_string() {
        let mut manager = UserInputManager::new();

        manager.insert_string("Hello World!");
        assert_eq!(&manager.user_input, "Hello World!");

        for chr in " How is it going?".chars() {
            manager.insert_char(chr);
        }
        assert_eq!(&manager.user_input, "Hello World! How is it going?");
    }

    #[test]
    fn can_move_cursor_left_right_remove() {
        let mut manager = UserInputManager::new();
        manager.insert_string("Hello World!");
        assert!(manager.cursor_position == "Hello World!".len());

        for _ in 0..7 {
            manager.move_cursor_left();
        }
        assert_eq!(manager.cursor_position, "Hello World!".len() - 7);

        manager.insert_char('#');
        assert_eq!(&manager.user_input, "Hello# World!");

        manager.move_cursor_right();
        assert_eq!(manager.cursor_position, "Hello# World!".len() - 6);

        manager.remove_char();
        assert_eq!(&manager.user_input, "Hello#World!");
    }

    #[test]
    fn can_autocomplete_single_command() {
        let mut manager = UserInputManager::new();
        let command = "FunnyCommand".to_owned();
        let library = [&command];

        manager.insert_string("Fun");

        let (prefix, matches) = manager.suggestion(&library);
        assert_eq!(prefix, "FunnyCommand");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], &command);
    }

    #[test]
    fn finds_longest_shared_prefix_of_commands() {
        let mut manager = UserInputManager::new();
        let command_a = "FunnyCommand".to_owned();
        let command_b = "Funnel".to_owned();
        let command_c = "Funtastic".to_owned();
        let command_not_matching = "Hamilton".to_owned();
        let library = [&command_a, &command_b, &command_c, &command_not_matching];
        let library_different_order = [&command_c, &command_not_matching, &command_a, &command_b];

        manager.insert_string("F");

        let (prefix, matches) = manager.suggestion(&library);
        assert_eq!(prefix, "Fun");
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0], &command_b);
        assert_eq!(matches[1], &command_a);
        assert_eq!(matches[2], &command_c);

        // Order is an implementation detail and should not be depended on!
        // For now, sorted alphabetically.
        // This test should fire if we ever change the suggestion algo to alter the order
        // based on the library ordering
        let (prefix, matches) = manager.suggestion(&library_different_order);
        assert_eq!(prefix, "Fun");
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0], &command_b);
        assert_eq!(matches[1], &command_a);
        assert_eq!(matches[2], &command_c);
    }

    #[test]
    fn can_set_suggestion_into_user_input_buffer() {
        let mut manager = UserInputManager::new();
        let command = "FunnyCommand".to_owned();
        let library = [&command];

        manager.insert_string("Fun");
        manager.set_suggest_completion(&library);
        assert_eq!(&manager.user_input, &command);
    }

    #[test]
    fn user_input_buffer_not_altered_when_no_suggestion_available() {
        let mut manager = UserInputManager::new();
        let command = "FunnyCommand".to_owned();
        let library = [&command];

        manager.insert_string("NotMatchingAnyCommand");
        manager.set_suggest_completion(&library);
        assert_eq!(manager.user_input, "NotMatchingAnyCommand");
        assert_ne!(&manager.user_input, &command);
    }

    #[test]
    fn finishing_input_clears_input_and_adds_to_history() {
        const USER_INPUT: &str = "This goes to the history!";
        let mut manager = UserInputManager::new();

        manager.insert_string(USER_INPUT);
        manager.finish_current_input();
        assert!(manager.input_empty());

        manager.set_to_previous_input();
        assert_eq!(manager.user_input, USER_INPUT);
    }

    #[test]
    fn finishing_empty_input_does_not_add_to_history() {
        const USER_INPUT: &str = "This goes to the history!";
        const EMPTY_USER_INPUT: &str = "";
        let mut manager = UserInputManager::new();

        manager.insert_string(USER_INPUT);
        manager.finish_current_input();
        manager.insert_string(EMPTY_USER_INPUT);
        manager.set_to_previous_input();
        assert_eq!(manager.user_input, USER_INPUT);
    }

    #[test]
    fn finishing_repeated_input_adda_to_history_exactly_once() {
        const USER_INPUT: &str = "This goes to the history!";
        const USER_INPUT_REPEATED: &str = "This goes to the history exactly once!";
        let mut manager = UserInputManager::new();

        manager.insert_string(USER_INPUT);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_REPEATED);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_REPEATED);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_REPEATED);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_REPEATED);
        manager.finish_current_input();
        manager.set_to_previous_input();
        assert_eq!(manager.user_input, USER_INPUT_REPEATED);
        manager.set_to_previous_input();
        assert_eq!(manager.user_input, USER_INPUT);
    }

    #[test]
    fn finishing_input_from_history_appends_to_the_end_of_the_history() {
        let mut manager = UserInputManager::new();

        for num in 1..=10 {
            manager.insert_string(&format!("Foo No. {num}"));
            manager.finish_current_input();
        }

        // Going back once puts us at Foo No. 10
        manager.set_to_previous_input();

        // Going back five times should put us at Foo No. 5
        for _ in 1..=5 {
            manager.set_to_previous_input();
        }
        assert_eq!(manager.user_input, "Foo No. 5");

        manager.insert_string(" (edit, this is now No. 11)");
        manager.finish_current_input();
        manager.set_to_previous_input();
        assert_eq!(manager.user_input, "Foo No. 5 (edit, this is now No. 11)");
    }

    #[test]
    fn can_not_overstep_history_at_beginning_nor_end() {
        const USER_INPUT_A: &str = "This goes to the history!";
        const USER_INPUT_B: &str = "This too goes to the history!";
        const USER_INPUT_C: &str = "So does this!";
        let mut manager = UserInputManager::new();

        manager.insert_string(USER_INPUT_A);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_B);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_C);
        manager.finish_current_input();

        manager.set_to_next_input();
        assert!(manager.input_empty());

        manager.set_to_previous_input(); // C
        manager.set_to_previous_input(); // B
        manager.set_to_previous_input(); // A
        manager.set_to_previous_input(); // -1
        manager.set_to_previous_input(); // -2
        assert_eq!(manager.user_input, USER_INPUT_A);
    }

    #[test]
    fn can_move_backward_and_forward_through_history() {
        const USER_INPUT_A: &str = "This goes to the history!";
        const USER_INPUT_B: &str = "This too goes to the history!";
        const USER_INPUT_C: &str = "So does this!";
        const USER_INPUT_D_NOT_COMMITED_YET: &str = "work in progress";
        let mut manager = UserInputManager::new();

        manager.insert_string(USER_INPUT_A);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_B);
        manager.finish_current_input();
        manager.insert_string(USER_INPUT_C);
        manager.finish_current_input();

        // Deliberately not finishing this input
        manager.insert_string(USER_INPUT_D_NOT_COMMITED_YET);

        // Backwards
        manager.set_to_previous_input(); // C
        assert_eq!(manager.user_input, USER_INPUT_C);
        manager.set_to_previous_input(); // B
        assert_eq!(manager.user_input, USER_INPUT_B);
        manager.set_to_previous_input(); // A
        assert_eq!(manager.user_input, USER_INPUT_A);

        // Forwards
        manager.set_to_next_input();
        assert_eq!(manager.user_input, USER_INPUT_B);
        manager.set_to_next_input();
        assert_eq!(manager.user_input, USER_INPUT_C);

        // Working copy is dismissed
        // Probably an implementation detail and not part of the API?
        manager.set_to_next_input();
        assert!(manager.input_empty());
        assert_ne!(manager.user_input, USER_INPUT_D_NOT_COMMITED_YET);
    }
}
