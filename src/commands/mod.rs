use std::cmp::Ordering;

use button_led::ButtonLed;
use coap_lite::CoapRequest;
use mem::MemRead;

use crate::commands::coap_get_template::CoapGet;
use crate::commands::multi_endpoints_sample::MultiEndpointSample;
use crate::commands::sample::SampleCommand;
use crate::commands::saul::Saul;
use crate::commands::wks::Wkc;

mod button_led;
mod coap_get_template;
mod mem;
mod multi_endpoints_sample;
mod sample;
mod saul;
mod wks;

/// Callback API for handling a command.
pub trait CommandHandler {
    /// This function is called exactly once. It is always the first call for any handler.
    /// Returns a coap request that is send to the attached device.
    fn init(&mut self) -> CoapRequest<String>;

    /// Called everytime a response is found to the last request this handler has sent.
    fn handle(&mut self, _payload: &[u8]) -> Option<CoapRequest<String>> {
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

    fn export(&self) -> Vec<u8> {
        let mut buffer = String::new();
        self.display(&mut buffer);
        buffer.as_bytes().to_vec()
    }
}

type BoxedCommandHandler = Box<dyn CommandHandler>;
/// Helper trait, used as glue between the library, command and handler, unifying the parsing.
pub trait CommandRegistry {
    /// Returns a new Command instance for this Handler
    fn cmd() -> Command;

    /// Parses a cli string, typically via clap
    ///
    /// On success, returns an implementation of the `CommandHandler` trait
    /// On error, returns a human readable usage error
    fn parse(cmd: &Command, args: String) -> Result<BoxedCommandHandler, String>;
}

/// Represents a command that the user can type into jelly
pub struct Command {
    /// The name of the command, this is what the user types into jelly.
    pub cmd: String,

    /// A description of what this command will do.
    pub description: String,

    /// The CoAP end-point(s) this command requires, if any.
    pub required_endpoints: Vec<String>,

    /// Parses a cli string, this typically a wrapper around the `CommandRegistry::parse()` function.
    ///
    /// On success, returns an implementation of the `CommandHandler` trait.
    /// On error, returns a human readable usage error.
    pub parse: fn(&Self, args: String) -> Result<BoxedCommandHandler, String>,
}

impl Command {
    /// Creates a new command without an end-point
    pub fn new(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            required_endpoints: vec![],
            parse: |_, _| Err("Undefined parse function".to_owned()),
        }
    }

    /// Creates a new command from an CoAP end-point / location.
    /// The location will become the commands name (what the user has to type in)
    pub fn from_coap_resource(resource: &str, description: &str) -> Self {
        let mut new = Self::new(resource, description);
        new.required_endpoints.push(resource.to_owned());
        new.parse = |s, a| CoapGet::parse(s, a);
        new
    }

    /// Creates a new command from a special, RIOT specific CoAP end-point.
    /// The location will be converted into the commands name (what the user has to type in)
    pub fn from_location(location: &str, description: &str) -> Self {
        let cmd = location
            .strip_prefix("/shell/")
            .expect("Failed to parse shell command location!");
        Self::new(cmd, description)
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
            cmds: vec![
                Command::new("help", "Prints all available commands"),
                Command::from_coap_resource("/.well-known/core", "Query the wkc"),
                Wkc::cmd(),
            ],
            stored_cmds: vec![
                ButtonLed::cmd(),
                SampleCommand::cmd(),
                Saul::cmd(),
                MultiEndpointSample::cmd(),
                MemRead::cmd(),
            ],
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
