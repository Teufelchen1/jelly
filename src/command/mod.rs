use std::cmp::Ordering;

use coap_lite::{CoapRequest, Packet};

use commands::CoapEdit;
use commands::CoapGet;
pub use commands::InternalCommand;
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
    Jelly(InternalCommand),
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

    /// Data that commands that carry state can store at creation time.
    ///
    /// (In a sense, this is a bypass for either .parse taking a closure, or commands having access
    /// to the App at execution time).
    ///
    /// This could be a box, but then every command that *does* need to carry the data around would
    /// need to do a `Box<Rc<_>>`, and we already know that we don't need Send, so this is cheap
    /// for the general case.
    state: Option<std::rc::Rc<dyn std::any::Any>>,
}

impl Command {
    /// Creates a new command without an end-point, send as raw diagnostic message
    pub fn new_text_type(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            required_endpoints: vec![],
            parse: |_c, cmd_str| Ok(CommandType::Text(cmd_str.to_owned())),
            state: None,
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
            state: None,
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

    /// Creates a command that, when run after a GET, opens an editor and PUTs it back up.
    ///
    /// This intentionally takes arguments that might not be successful: When something goes wrong
    /// constructing the cache state, it's better to fail here than not have the new command
    /// created at all (and the old edit command linger) -- this is to ensure that if a user GETs
    /// /a, then GETs /b unsuccessfully and tris to edit it, they don't accidentally edit /a.
    pub fn new_edit(request: &CoapRequest<String>, response: &Packet) -> Self {
        Self {
            cmd: "edit".to_string(),
            description: "Performs a read-modify-write operation on the last used resource"
                .to_string(),
            required_endpoints: vec![],
            parse: |c, a| CoapEdit::parse(c, a),
            state: Some(std::rc::Rc::new(std::cell::RefCell::new(
                CoapEdit::state_for(request, response),
            ))),
        }
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
