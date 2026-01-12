use super::Command;
use super::commands::all_commands;

/// The command library maintains all known commands and provides easy access to them
pub struct CommandLibrary {
    /// Available commands
    cmds: Vec<Command>,
    /// Known commands
    stored_cmds: Vec<Command>,
}
impl CommandLibrary {
    /// Create a new library with only the two default commands included
    /// - help: Prints all available commands
    /// - /.well-known/core: Query the wkc
    pub fn default() -> Self {
        Self {
            cmds: vec![],
            stored_cmds: all_commands(),
        }
    }

    /// Takes a list of endpoints that are available, this is typically the list received
    /// from /.well-known/core, goes through all `.stored_cmds` and sees if of any of them
    /// the requirements are met. If so, they are added to the available `.cmds`
    /// Finally, the available `.cmds` are re-sorted.
    pub fn update_available_cmds_based_on_endpoints(&mut self, eps: &[String]) {
        let mut i = 0;
        while i < self.stored_cmds.len() {
            let scmd = &self.stored_cmds[i];
            // Todo: Commands should be able to decide on their own if they are available
            if scmd.required_endpoints.iter().all(|ep| eps.contains(ep)) {
                self.cmds.push(self.stored_cmds.remove(i));
            } else {
                i += 1;
            }
        }

        self.cmds.sort();
    }

    pub fn force_all_cmds_available(&mut self) {
        while !self.stored_cmds.is_empty() {
            self.cmds.push(self.stored_cmds.remove(0));
        }

        self.cmds.sort();
    }

    /// Returns a list of the `Command.cmd` of all known `Command`s
    pub fn list_by_cmd(&self) -> Vec<String> {
        self.cmds.iter().map(|x| x.cmd.clone()).collect()
    }

    pub fn list_available_commands(&self) -> Vec<&Command> {
        self.cmds.iter().collect()
    }

    /// Adds a `Command`
    pub fn add(&mut self, cmd: Command) {
        self.cmds.push(cmd);
    }

    /// Returns all `Command`s whos `.cmd` matches the given prefix
    pub fn matching_prefix_by_cmd(&self, prefix: &str) -> Vec<&Command> {
        self.cmds
            .iter()
            .filter(|known_cmd| known_cmd.starts_with(prefix))
            .collect()
    }

    /// Takes a given prefix, computes all `Command`s that match it.
    /// The prefix is then prolonged as long as the list of matching `Command`s stays identical
    /// For example if the given prefix `F` matches `FooBar`, `FooBaz` and `FooBizz`, this
    /// function would return '`(FooB, [FooBar, FooBaz, FooBizz])`'
    pub fn longest_common_prefixed_by_cmd(&self, prefix: &str) -> (String, Vec<&Command>) {
        let cmds = self.matching_prefix_by_cmd(prefix);

        let actual_prefix = match cmds.len() {
            0 => prefix.to_owned(),
            1 => cmds[0].cmd.clone(),
            _ => {
                let mut common_prefix = prefix.to_owned();
                let first_cmd = &cmds[0].cmd;
                'outer: for (i, character) in first_cmd.chars().enumerate().skip(prefix.len()) {
                    for othercmd in cmds.iter().skip(1) {
                        if i >= othercmd.cmd.len() || othercmd.cmd.chars().nth(i) != Some(character)
                        {
                            break 'outer;
                        }
                    }
                    common_prefix.push(character);
                }
                common_prefix
            }
        };
        (actual_prefix, cmds)
    }

    /// Finds the `Command` whose `cmd` matches exactly the input
    pub fn find_by_cmd(&self, cmd: &str) -> Option<&Command> {
        let cmd_clean = cmd.trim_ascii_end();
        self.cmds
            .iter()
            .find(|known_cmd| known_cmd.cmd == cmd_clean)
    }

    /// Finds the `Command` whose `cmd` matches exactly the input, returns mutable
    pub fn find_by_cmd_mut(&mut self, cmd: &str) -> Option<&mut Command> {
        let cmd_clean = cmd.trim_ascii_end();
        self.cmds
            .iter_mut()
            .find(|known_cmd| known_cmd.cmd == cmd_clean)
    }

    /// Finds the `Command` whose `location` matches exactly the input
    pub fn find_by_first_location(&self, location: &str) -> Option<&Command> {
        self.cmds.iter().find(|known_cmd| {
            known_cmd
                .required_endpoints
                .first()
                .is_some_and(|l| l == location)
        })
    }

    /// Same as `find_by_location` but returns a mutable
    pub fn find_by_first_location_mut(&mut self, location: &str) -> Option<&mut Command> {
        self.cmds.iter_mut().find(|known_cmd| {
            known_cmd
                .required_endpoints
                .first()
                .is_some_and(|l| l == location)
        })
    }

    /// Checks is a given `Command` is already in the library
    pub fn _contains(&self, cmd: &Command) -> bool {
        for known_cmd in &self.cmds {
            if known_cmd == cmd {
                return true;
            }
        }
        false
    }
}
