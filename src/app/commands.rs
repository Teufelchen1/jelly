use std::fmt::Write;

use clap::Parser;
use clap::Subcommand;
use minicbor::Decoder;
use minicbor::Encoder;

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
                Command {
                    cmd: "SampleCommand".to_owned(),
                    description: "An example coap based command".to_owned(),
                    location: Some("/SampleCommand".to_owned()),
                    handler: Some(sample_command_handler),
                    display: Some(sample_command_display),
                },
                Command {
                    cmd: "Saul".to_owned(),
                    description: "Saul over coap".to_owned(),
                    location: Some("/Saul".to_owned()),
                    handler: Some(saul_handler),
                    display: Some(saul_display),
                },
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
    pub fn find_by_cmd(&self, cmd: &str) -> Option<&Command> {
        let cmd_clean = cmd.trim_ascii_end();
        self.cmds
            .iter()
            .find(|known_cmd| known_cmd.cmd == cmd_clean)
    }

    /// Finds the `Command` whose `cmd` matches exactly the input, returns mutable
    pub fn _find_by_cmd_mut(&mut self, cmd: &str) -> Option<&mut Command> {
        let cmd_clean = cmd.trim_ascii_end();
        self.cmds
            .iter_mut()
            .find(|known_cmd| known_cmd.cmd == cmd_clean)
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

type Handler = fn(String) -> Result<Vec<u8>, String>;
type Displayer = fn(Vec<u8>) -> String;

/// Represents a command that the user can type into jelly
pub struct Command {
    /// The name of the command, this is what the user types into jelly
    pub cmd: String,
    /// A description of what this command will do
    pub description: String,
    /// The CoAP end-point this command belongs to, if any
    pub location: Option<String>,

    pub handler: Option<Handler>,

    pub display: Option<Displayer>,
}

impl Command {
    /// Creates a new command without an end-point
    pub fn new(cmd: &str, description: &str) -> Self {
        Self {
            cmd: cmd.to_owned(),
            description: description.to_owned(),
            location: None,
            handler: None,
            display: None,
        }
    }

    /// Creates a new command from an CoAP end-point / location.
    /// The location will become the commands name (what the user has to type in)
    pub fn from_coap_resource(resource: &str, description: &str) -> Self {
        Self {
            cmd: resource.to_owned(),
            description: description.to_owned(),
            location: Some(resource.to_owned()),
            handler: None,
            display: None,
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
            handler: None,
            display: None,
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

#[derive(Parser, Debug)]
#[command(name = "SampleCommand")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is an example command")]
pub struct SampleCommand {
    #[arg(long)]
    caps: bool,
    #[arg(long, default_value_t = 1)]
    repeats: usize,
}

pub fn sample_command_display(payload: Vec<u8>) -> String {
    let buffer = String::from_utf8_lossy(&payload);
    buffer.replace(" ", "\n")
}

pub fn sample_command_handler(args: String) -> Result<Vec<u8>, String> {
    let cmd = SampleCommand::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;

    let mut buffer: [u8; 4] = [0; 4];
    let mut encoder = Encoder::new(&mut buffer[..]);

    let _ = encoder
        .array(2)
        .unwrap()
        .bool(cmd.caps)
        .unwrap()
        .u8(cmd.repeats.try_into().unwrap())
        .unwrap()
        .end();
    Ok(buffer.to_vec())
}

#[derive(Parser, Debug)]
#[command(name = "Saul")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is saul over coap")]
pub struct Saul {
    #[command(subcommand)]
    operation: Option<SaulOperation>,
}

#[derive(Subcommand, Debug)]
enum SaulOperation {
    Read { id: u8 },
    Write { id: u8, data: u8 },
}

pub fn saul_display(payload: Vec<u8>) -> String {
    const SAUL_CAT_MASK: u8 = 0xc0u8;
    /**< Bitmask to obtain the category ID */
    const SAUL_ID_MASK: u8 = 0x3fu8;
    /**< Bitmask to obtain the intra-category ID */

    const SAUL_CAT_UNDEF: u8 = 0x00;
    /**< device class undefined */
    const SAUL_CAT_ACT: u8 = 0x40;
    /**< Actuator device class */
    const SAUL_CAT_SENSE: u8 = 0x80;
    /**< Sensor device class */

    const SENSE_NAME: [&str; 29] = [
        "SAUL_SENSE_ID_ANY",
        "SAUL_SENSE_ID_BTN",
        "SAUL_SENSE_ID_TEMP",
        "SAUL_SENSE_ID_HUM",
        "SAUL_SENSE_ID_LIGHT",
        "SAUL_SENSE_ID_ACCEL",
        "SAUL_SENSE_ID_MAG",
        "SAUL_SENSE_ID_GYRO",
        "SAUL_SENSE_ID_COLOR",
        "SAUL_SENSE_ID_PRESS",
        "SAUL_SENSE_ID_ANALOG",
        "SAUL_SENSE_ID_UV",
        "SAUL_SENSE_ID_OBJTEMP",
        "SAUL_SENSE_ID_COUNT",
        "SAUL_SENSE_ID_DISTANCE",
        "SAUL_SENSE_ID_CO2",
        "SAUL_SENSE_ID_TVOC",
        "SAUL_SENSE_ID_GAS",
        "SAUL_SENSE_ID_OCCUP",
        "SAUL_SENSE_ID_PROXIMITY",
        "SAUL_SENSE_ID_RSSI",
        "SAUL_SENSE_ID_CHARGE",
        "SAUL_SENSE_ID_CURRENT",
        "SAUL_SENSE_ID_PM",
        "SAUL_SENSE_ID_CAPACITANCE",
        "SAUL_SENSE_ID_VOLTAGE",
        "SAUL_SENSE_ID_PH",
        "SAUL_SENSE_ID_POWER",
        "SAUL_SENSE_ID_SIZE",
    ];

    const ACT_NAME: [&str; 7] = [
        "SAUL_ACT_ID_ANY",
        "SAUL_ACT_ID_LED_RGB",
        "SAUL_ACT_ID_SERVO",
        "SAUL_ACT_ID_MOTOR",
        "SAUL_ACT_ID_SWITCH",
        "SAUL_ACT_ID_DIMMER",
        "SAUL_ACT_NUMOF",
    ];

    let mut out = String::new();

    let mut decoder = Decoder::new(&payload);
    decoder.array().unwrap();
    match decoder.u8().unwrap() {
        // List
        0 => {
            decoder.array().unwrap();
            while decoder.probe().array().is_ok() {
                decoder.array().unwrap();
                let id = decoder.u8().unwrap();
                let class = decoder.u8().unwrap();
                let name = decoder.str().unwrap();

                let class_name = match class & SAUL_CAT_MASK {
                    SAUL_CAT_UNDEF => "UNDEFINED",
                    SAUL_CAT_ACT => ACT_NAME[(class & SAUL_ID_MASK) as usize],
                    SAUL_CAT_SENSE => SENSE_NAME[(class & SAUL_ID_MASK) as usize],
                    _ => todo!(),
                };
                let _ = write!(out, "{id}, {class_name}, {name}\n");
            }
        }
        // read
        1 => {
            let data = decoder.map();
            match data {
                Ok(_data) => {
                    let _ = write!(
                        out,
                        "{}\n",
                        cbor_edn::StandaloneItem::from_cbor(&payload[2..]).map_or_else(
                            |e| format!("Parsing error {e}, content {payload:02x?}\n"),
                            |c| c.serialize(),
                        )
                    );
                }
                Err(_) => {
                    let _ = write!(out, "No device with this id found\n");
                }
            }
        }
        // write
        2 => {}
        _ => {
            todo!();
        }
    }
    out
}

pub fn saul_handler(args: String) -> Result<Vec<u8>, String> {
    let cmd = Saul::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;

    let mut buffer: [u8; 12] = [0; 12];
    let mut encoder = Encoder::new(&mut buffer[..]);

    let _subcommand = {
        match cmd.operation {
            None => encoder.array(1).unwrap().u8(0).unwrap().end(),
            Some(SaulOperation::Read { id }) => encoder
                .array(2)
                .unwrap()
                .u8(1)
                .unwrap()
                .u8(id)
                .unwrap()
                .end(),
            Some(SaulOperation::Write { id, data }) => encoder
                .array(3)
                .unwrap()
                .u8(2)
                .unwrap()
                .u8(id)
                .unwrap()
                .u8(data)
                .unwrap()
                .end(),
        }
    };

    Ok(buffer.to_vec())
}
