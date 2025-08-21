use clap::Args;
use clap::Parser;
use clap::Subcommand;
use std::fmt;

use super::Command;
use super::CommandRegistry;
use super::HandlerType;

#[derive(Parser, Debug)]
#[command(name = "Ifconfig")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "\n
This is a wrapper around `ifconfig`.
It provides a modern cli that gets converted to the regular RIOT ifconfig command.")]
pub struct IfconfigCli {
    id: Option<u8>,

    #[command(subcommand)]
    operation: Option<Operation>,
}

#[derive(Subcommand, Debug)]
enum Operation {
    // Sets a hardware specific value
    Set(SetArgs),
    Add,
    Del,
    Stats,
    Up,
    Down,
}

#[derive(Args, Debug)]
struct SetArgs {
    #[command(subcommand)]
    key: SetOperation,
}

#[derive(Subcommand, Debug)]
enum SetOperation {
    /// sets (short) address
    Addr { value: String },
    /// sets long address
    Addr_long { value: String },
    /// alias for "addr"
    Addr_short { value: String },
    /// set ED threshold during CCA in dBm
    Cca_threshold { value: String },
    /// sets the "channel" center frequency
    Freq { value: String },
    /// sets the frequency channel
    Channel { value: String },
    /// alias for "channel"
    Chan { value: String },
    /// set checksumming on-off
    Checksum { value: String },
    /// set max. number of channel access attempts
    Csma_retries { value: String },
    /// set the encryption on-off
    Encrypt { value: String },
    /// set hop limit
    Hop_limit { value: String },
    /// alias for "hop_limit"
    Hl { value: String },
    /// set the encryption key in hexadecimal format
    Key { value: String },
    /// IPv6 maximum transmission unit
    Mtu { value: String },
    /// sets the network identifier (or the PAN ID)
    Nid { value: String },
    /// set the channel page (IEEE 802.15.4)
    Page { value: String },
    /// alias for "nid"
    Pan { value: String },
    /// alias for "nid"
    Pan_id { value: String },
    /// set busy mode on-off
    Phy_busy { value: String },
    /// TX power in dBm
    Power { value: String },
    /// max. number of retransmissions
    Retrans { value: String },
    /// sets the source address length in byte
    Src_len { value: String },
    /// set the device state
    State { value: String },
}

impl fmt::Display for SetOperation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct IfconfigWrapper {}

/// Interface with the library and handler
impl CommandRegistry for IfconfigWrapper {
    fn cmd() -> Command {
        Command {
            cmd: "Ifconfig".to_owned(),
            description: "ifconfig wrapper with better usage".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/.well-known/core".to_owned(), "/shell/ifconfig".to_owned()],
        }
    }

    // Saves the first path of this command...so this won't work with commands that need multiple.
    fn parse(_cmd: &Command, args: String) -> Result<HandlerType, String> {
        let cli =
            IfconfigCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;

        match (cli.id, cli.operation) {
            (Some(id), Some(operation)) => {
                let cmd_str = match operation {
                    Operation::Set(args) => {
                        format!("set {:} {:}", args.key.to_string(), args.key)
                    }
                    _ => todo!(),
                };
                let ifconfig = format!("ifconfig {id} {cmd_str}\n");
                Ok(HandlerType::DiagnosticMsg(ifconfig))
            }
            (Some(id), None) => Ok(HandlerType::DiagnosticMsg(
                format!("ifconfig {id}").to_owned(),
            )),
            (None, Some(operation)) => Ok(HandlerType::DiagnosticMsg("ifconfig --help".to_owned())),
            (None, None) => Ok(HandlerType::DiagnosticMsg("ifconfig --help".to_owned())),
        }
    }
}
