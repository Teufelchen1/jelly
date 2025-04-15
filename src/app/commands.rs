pub struct CommandLibrary {
    cmds: Vec<Command>,
}
impl CommandLibrary {
    pub fn default() -> Self {
        Self {
            cmds: vec![
                Command::new("help", "Prints all available commands"),
                Command::from_coap_resource("/.well-known/core", "Query the wkc"),
            ],
        }
    }

    pub fn list_by_cmd(&self) -> Vec<String> {
        self.cmds.iter().map(|x| x.cmd.clone()).collect()
    }

    pub fn add(&mut self, cmd: Command) {
        self.cmds.push(cmd);
    }

    pub fn matching_prefix_by_cmd(&self, cmd: &str) -> Vec<&Command> {
        self.cmds
            .iter()
            .filter(|known_cmd| known_cmd.starts_with(cmd)).collect()
    }

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
                        if i >= othercmd.cmd.len() || othercmd.cmd.chars().nth(i) != Some(character) {
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

    pub fn _find_by_cmd(&self, cmd: &str) -> Option<&Command> {
        self.cmds.iter().find(|known_cmd| known_cmd.cmd == cmd)
    }

    pub fn _find_by_cmd_mut(&mut self, cmd: &str) -> Option<&mut Command> {
        self.cmds.iter_mut().find(|known_cmd| known_cmd.cmd == cmd)
    }

    pub fn find_by_location(&self, location: &str) -> Option<&Command> {
        self.cmds
            .iter()
            .find(|known_cmd| known_cmd.location.as_ref().is_some_and(|l| l == location))
    }

    pub fn find_by_location_mut(&mut self, location: &str) -> Option<&mut Command> {
        self.cmds
            .iter_mut()
            .find(|known_cmd| known_cmd.location.as_ref().is_some_and(|l| l == location))
    }

    pub fn _contains(&self, cmd: &Command) -> bool {
        for known_cmd in &self.cmds {
            if known_cmd == cmd {
                return true;
            }
        }
        false
    }
}

pub struct Command {
    pub cmd: String,
    pub description: String,
    pub location: Option<String>,
}
impl Command {
    pub fn new(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            location: None,
        }
    }

    pub fn from_coap_resource(resource: &str, description: &str) -> Self {
        Self {
            cmd: resource.to_owned(),
            description: description.to_owned(),
            location: Some(resource.to_owned()),
        }
    }

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

    pub fn update_description(&mut self, description: &str) {
        self.description.clear();
        self.description.push_str(description);
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.cmd.starts_with(prefix)
    }
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.cmd == other.cmd
    }
}
