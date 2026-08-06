use std::fmt::Write;

use clap::Parser;
use clap::Subcommand;
use coap_lite::RequestType as Method;
use coap_lite::{CoapRequest, Packet};
use coap_message::MinimalWritableMessage;
use minicbor::Decoder;
use minicbor::Encoder;
//use senml;

use super::Command;
use super::CommandHandler;
use super::CommandType;

#[derive(Clone, Debug)]
struct MyIpv6Addr {
    addr: std::net::Ipv6Addr,
    prefix: u8,
}

impl std::str::FromStr for MyIpv6Addr {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let mut splitted = s.split('/');
        if let Some(addr) = splitted.next() {
            let addr = std::net::Ipv6Addr::from_str(addr).map_err(|e| e.to_string())?;
            let prefix = if let Some(prefix) = splitted.next() {
                u8::from_str(prefix).map_err(|e| e.to_string())?
            } else {
                128
            };
            Ok(Self { addr, prefix })
        } else {
            Err("No ipv6 addr provided".to_string())
        }
    }
}

/// This is an example on how to use cbor as payload for the coap request.
#[derive(Parser, Debug)]
#[command(name = "Saul")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is saul over coap")]
pub struct IpCli {
    #[command(subcommand)]
    operation: Option<IpOperation>,
}

#[derive(Subcommand, Debug)]
enum IpOperation {
    /// Lists all attached sensors and actuators (this is the default)
    List,
    /// Read the values from the specified interface
    Read {
        #[arg(required = true, num_args = 1.., value_delimiter = ' ')]
        interface_names: Vec<String>,
    },
    /// Write a 8 bit value into an actuator
    AddrAdd { addr: MyIpv6Addr, iface: String },
}

struct Ip {
    location: String,
    buffer: String,
    payload: Vec<u8>,
    finished: bool,
    displayable: bool,
    cli: IpCli,
}

pub fn cmd() -> Command {
    Command {
        cmd: "ip".to_owned(),
        description: "ifconfig over coap".to_owned(),
        parse: |s, a| parse(s, a),
        required_endpoints: vec!["/jelly/netif".to_owned()],
    }
}

fn parse(cmd: &Command, args: &str) -> Result<CommandType, String> {
    let cli = IpCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
    Ok(CommandType::CoAP(Box::new(Ip {
        location: cmd.required_endpoints[0].clone(),
        buffer: String::new(),
        payload: vec![],
        finished: false,
        displayable: false,
        cli,
    })))
}

impl CommandHandler for Ip {
    fn init(&mut self) -> CoapRequest<String> {
        let mut buffer: [u8; 64] = [0; 64];
        let mut encoder = Encoder::new(&mut buffer[..]);

        let request = match &self.cli.operation {
            None | Some(IpOperation::List) => {
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path(&self.location);
                request
            }
            Some(IpOperation::Read { interface_names }) => {
                encoder
                    .array(interface_names.len().try_into().unwrap())
                    .unwrap();
                for iface in interface_names {
                    encoder.str(iface).unwrap();
                }

                encoder.end().unwrap();
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path(&self.location);
                request
                    .message
                    .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
                request.message.set_payload(&buffer).unwrap();
                request
            }
            Some(IpOperation::AddrAdd { addr, iface }) => {
                let addr_octs = addr.addr.octets();
                encoder
                    .array(2)
                    .unwrap()
                    .tag(minicbor::data::Tag::new(20))
                    .unwrap()
                    .str(iface)
                    .unwrap()
                    .tag(minicbor::data::Tag::new(54))
                    .unwrap()
                    .array(2)
                    .unwrap()
                    .bytes(&addr_octs)
                    .unwrap()
                    .u8(addr.prefix)
                    .unwrap();
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Post);
                request.set_path(&self.location);
                request
                    .message
                    .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
                request.message.set_payload(&buffer).unwrap();
                request
            }
        };

        request
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        self.payload.clone_from(&response.payload);
        let mut out = String::new();

        match self.cli.operation {
            None | Some(IpOperation::List) => {
                out = decode_netif_list_into_string(&self.payload);
            }
            Some(IpOperation::Read { interface_names: _ }) => {
                out = decode_netif_list_into_string(&self.payload);
            }
            Some(IpOperation::AddrAdd { addr: _, iface: _ }) => {
                // no op
            }
        }
        self.buffer = out;
        self.finished = true;
        self.displayable = true;
        None
    }

    fn want_display(&self) -> bool {
        self.displayable
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn display(&self, buffer: &mut String) {
        let _ = writeln!(buffer, "{}", self.buffer);
    }

    fn export(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

fn decode_netif_into_string(decoder: &mut Decoder) -> String {
    fn get_ipv6(data: &[u8]) -> std::net::Ipv6Addr {
        let ip = std::net::Ipv6Addr::from(
            <&[u8] as TryInto<[u8; 16]>>::try_into(data).expect("Ipv6 should be 16 bytes long"),
        );
        ip
    }
    fn print_mac(out: &mut String, data: &[u8]) {
        let last_i = data.len() - 1;
        for i in 0..last_i {
            write!(out, "{:02X}:", data[i]).unwrap();
        }
        write!(out, "{:02X}", data[last_i]).unwrap();
    }

    let mut out = String::new();
    while decoder.probe().tag().is_ok() {
        match decoder.tag().unwrap().as_u64() {
            20 => {
                write!(out, "Iface {}\n", decoder.str().unwrap()).unwrap();
            }
            48 => {
                write!(out, "   HWaddr: ").unwrap();
                print_mac(&mut out, decoder.bytes().unwrap());
                write!(out, "\n").unwrap();
            }
            54 => {
                write!(out, "   inet6 ").unwrap();
                if decoder.probe().array().is_ok() {
                    decoder.array().unwrap();
                    if let Ok(prefix) = decoder.u8() {
                        let ip = get_ipv6(decoder.bytes().unwrap());
                        let _ = write!(out, "group: {ip}/{prefix}");
                    } else {
                        let ip = get_ipv6(decoder.bytes().unwrap());
                        let _ = write!(out, "addr: {ip}");
                        if let Ok(prefix) = decoder.u8() {
                            let _ = write!(out, "/{prefix}");
                        }
                    }
                } else {
                    let ip = get_ipv6(decoder.bytes().unwrap());
                    let _ = write!(out, "addr: {ip}");
                }
                writeln!(out).unwrap();
            }
            302 => {
                if decoder.bool().unwrap() {
                    writeln!(out, "   Link type: wired").unwrap();
                } else {
                    writeln!(out, "   Link type: wireless").unwrap();
                }
            }
            _ => continue,
        }
    }
    out
}

fn decode_netif_list_into_string(data: &[u8]) -> String {
    let mut out = String::new();
    let mut decoder = Decoder::new(data);

    decoder.array().unwrap();
    {
        while decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            let _ = writeln!(out, "{}", decode_netif_into_string(&mut decoder));
            if let Ok(minicbor::data::Type::Break) = decoder.probe().datatype() {
                decoder.skip().unwrap();
            }
        }
    }
    out
}
