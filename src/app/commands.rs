pub struct Command {
    pub cmd: String,
    pub description: String,
    pub _location: Option<String>,
}
impl Command {
    pub fn new(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            _location: None,
        }
    }

    pub fn new_coap_resource(resource: &str, description: &str) -> Self {
        Self {
            cmd: resource.to_owned(),
            description: description.to_owned(),
            _location: Some(resource.to_owned()),
        }
    }

    pub fn from_location(location: &str, description: &str) -> Self {
        let cmd = location
            .strip_prefix("/shell/")
            .expect("Failed to parse shell command location!");
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            _location: Some(location.to_owned()),
        }
    }
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.cmd == other.cmd
    }
}
