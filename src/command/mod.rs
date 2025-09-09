use std::cmp::Ordering;

use coap_lite::{CoapRequest, Packet};

use commands::CoapGet;
pub use library::CommandLibrary;

mod commands;
mod library;

type BoxedCommandHandler = Box<dyn CommandHandler>;

/// The type of the command, Jellys behaviour towards that command depons on this
pub enum CommandType {
    /// A raw diagnostic text message, will be send as is to the device
    Text(String),
    /// A command that does CoAP interactions
    CoAP(BoxedCommandHandler),
    /// A Jelly internal command, e.g. Help
    Jelly,
}

/// Callback API for handling a command.
pub trait CommandHandler {
    /// This function is called exactly once. It is always the first call for any handler.
    /// Returns a coap request that is send to the attached device.
    fn init(&mut self) -> CoapRequest<String>;

    /// Called everytime a response is found to the last request this handler has sent.
    fn handle(&mut self, _payload: &Packet) -> Option<CoapRequest<String>> {
        None
    }

    /// Asks the handler if it wants to display anything. Usually called after a response was
    /// processed.
    fn want_display(&self) -> bool {
        false
    }

    /// Asks if the handler has completed its task and might get removed from the tracking.
    fn is_finished(&self) -> bool {
        true
    }

    /// Provides a buffer into which the handler can write the result.
    fn display(&self, _buffer: &mut String) {}

    /// Asks the command to provide a 'reasonable' binary export of its content
    fn export(&self) -> Vec<u8> {
        let mut buffer = String::new();
        self.display(&mut buffer);
        buffer.as_bytes().to_vec()
    }
}

/// Represents a command that the user can type into jelly
pub struct Command {
    /// The name of the command, this is what the user types into jelly.
    pub cmd: String,

    /// A description of what this command will do.
    pub description: String,

    /// The CoAP end-point(s) this command requires, if any.
    pub required_endpoints: Vec<String>,

    /// Parses a cli string.
    ///
    /// On success, returns a `CommandType`.
    /// On error, returns a human readable usage error.
    pub parse: fn(&Self, args: &str) -> Result<CommandType, String>,
}

impl Command {
    /// Creates a new command without an end-point, send as raw diagnostic message
    pub fn new_text_type(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            required_endpoints: vec![],
            parse: |c, _a| Ok(CommandType::Text(c.cmd.clone())),
        }
    }

    /// Creates a new command for Jelly internal usage
    pub fn new_jelly_type(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            required_endpoints: vec![],
            parse: |_c, _a| Ok(CommandType::Jelly),
        }
    }

    /// Creates a new command that performs a CoAP GET request to a resource location.
    /// The location will become the commands name (what the user has to type in)
    pub fn new_coap_get(resource: &str, description: &str) -> Self {
        Self {
            cmd: resource.to_owned(),
            description: description.to_owned(),
            required_endpoints: vec![resource.to_owned()],
            parse: |c, a| Ok(CommandType::CoAP(CoapGet::parse(c, a))),
        }
    }

    /// Creates a new command from a special, RIOT specific CoAP end-point.
    /// The location will be converted into the commands name (what the user has to type in)
    pub fn from_location(location: &str, description: &str) -> Self {
        let cmd = location
            .strip_prefix("/shell/")
            .expect("Failed to parse shell command location!");
        Self::new_text_type(cmd, description)
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

impl Eq for Command {}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.cmd == other.cmd
    }
}

impl PartialOrd for Command {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Command {
    fn cmp(&self, other: &Self) -> Ordering {
        let scmd = &self.cmd;
        let ocmd = &other.cmd;

        // Ensure that the help command is always the first one
        if scmd == "help" {
            return Ordering::Less;
        } else if ocmd == "help" {
            return Ordering::Greater;
        }

        // Sort direct coap requests (not really commands) to the back
        // Regular commands that requiere endpoints are sorted to the front
        // This ensures that jelly sided commands are sorted before riots commands
        match (scmd.starts_with('/'), ocmd.starts_with('/')) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (true, true) => scmd.cmp(ocmd),
            (false, false) => other
                .required_endpoints
                .len()
                .cmp(&self.required_endpoints.len()),
        }
    }
}
