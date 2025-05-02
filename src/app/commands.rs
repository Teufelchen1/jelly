/// The command library maintains all known commands and provides easy access to them
pub struct CommandLibrary {
    cmds: Vec<Command>,
}
impl CommandLibrary {
    /// Create a new library with only the two default commands included
    /// - help: Prints all available commands
    /// - /.well-known/core: Query the wkc
    pub fn default() -> Self {
        Self {
            cmds: vec![
                Command::new("help", "Prints all available commands"),
                Command::from_coap_resource("/.well-known/core", "Query the wkc"),
            ],
        }
    }

    /// Returns a list of the `Command.cmd` of all known `Command`s
    pub fn list_by_cmd(&self) -> Vec<String> {
        self.cmds.iter().map(|x| x.cmd.clone()).collect()
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
    pub fn _find_by_cmd(&self, cmd: &str) -> Option<&Command> {
        self.cmds.iter().find(|known_cmd| known_cmd.cmd == cmd)
    }

    /// Finds the `Command` whose `cmd` matches exactly the input, returns mutable
    pub fn _find_by_cmd_mut(&mut self, cmd: &str) -> Option<&mut Command> {
        self.cmds.iter_mut().find(|known_cmd| known_cmd.cmd == cmd)
    }

    /// Finds the `Command` whose `location` matches exactly the input
    pub fn find_by_location(&self, location: &str) -> Option<&Command> {
        self.cmds
            .iter()
            .find(|known_cmd| known_cmd.location.as_ref().is_some_and(|l| l == location))
    }

    /// Same as `find_by_location` but returns a mutable
    pub fn find_by_location_mut(&mut self, location: &str) -> Option<&mut Command> {
        self.cmds
            .iter_mut()
            .find(|known_cmd| known_cmd.location.as_ref().is_some_and(|l| l == location))
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

/// Represents a command that the user can type into jelly
pub struct Command {
    /// The name of the command, this is what the user types into jelly
    pub cmd: String,
    /// A description of what this command will do
    pub description: String,
    /// The CoAP end-point this command belongs to, if any
    pub location: Option<String>,
}

impl Command {
    /// Creates a new command without an end-point
    pub fn new(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            location: None,
        }
    }

    /// Creates a new command from an CoAP end-point / location.
    /// The location will become the commands name (what the user has to type in)
    pub fn from_coap_resource(resource: &str, description: &str) -> Self {
        Self {
            cmd: resource.to_owned(),
            description: description.to_owned(),
            location: Some(resource.to_owned()),
        }
    }

    /// Creates a new command from a special, RIOT specific CoAP end-point.
    /// The location will be converted into the commands name (what the user has to type in)
    pub fn from_location(location: &str, description: &str) -> Self {
        let cmd = location
            .strip_prefix("/shell/")
            .expect("Failed to parse shell command location!");
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            location: Some(location.to_owned()),
        }
    }

    /// Replaces the decription of a command
    pub fn update_description(&mut self, description: &str) {
        self.description.clear();
        self.description.push_str(description);
    }

    /// Checks if the name of this command matches a prefix
    fn starts_with(&self, prefix: &str) -> bool {
        self.cmd.starts_with(prefix)
    }
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.cmd == other.cmd
    }
}
