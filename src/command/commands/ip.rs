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

/// This is an example on how to use cbor as payload for the coap request.
#[derive(Parser, Debug)]
#[command(name = "Ifconfig")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is ifconfig over coap")]
pub struct IfconfigCli {
    iface: Option<String>,

    #[command(subcommand)]
    operation: Option<IfconfigOperation>,
}

#[derive(Subcommand, Debug)]
enum IfconfigOperation {
    Up,
    Down,
    Set {
        key: String,
        value: String,
    },
    /// Add an IPv6 address to the interface
    Add {
        addr: Ipv6AddrCidr,
    },
    /// Delete an IPv6 address from the interface
    Del {
        addr: Ipv6AddrCidr,
    },
}

struct Ifconfig {
    location: String,
    buffer: String,
    payload: Vec<u8>,
    finished: bool,
    displayable: bool,
    cli: IfconfigCli,
}

pub fn cmd() -> Command {
    Command {
        cmd: "Ifconfig".to_owned(),
        description: "ifconfig over coap".to_owned(),
        parse: |s, a| parse(s, a),
        required_endpoints: vec!["/jelly/netif".to_owned()],
    }
}

fn parse(cmd: &Command, args: &str) -> Result<CommandType, String> {
    let cli = IfconfigCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
    if cli.operation.is_some() && cli.iface.is_none() {
        return Err("a subcommand can only be used when an interface is given".to_string());
    }
    Ok(CommandType::CoAP(Box::new(Ifconfig {
        location: cmd.required_endpoints[0].clone(),
        buffer: String::new(),
        payload: vec![],
        finished: false,
        displayable: false,
        cli,
    })))
}

impl CommandHandler for Ifconfig {
    fn init(&mut self) -> CoapRequest<String> {
        let mut buffer: Vec<u8> = vec![];
        let mut encoder = Encoder::new(&mut buffer);

        let request = if let Some(iface_id) = &self.cli.iface {
            if let Some(operation) = &self.cli.operation {
                match operation {
                    IfconfigOperation::Add { addr } => {
                        let addr_octs = addr.addr.octets();
                        encoder
                            .array(2)
                            .unwrap()
                            .tag(minicbor::data::Tag::new(20))
                            .unwrap()
                            .str(iface_id)
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
                    IfconfigOperation::Del { addr } => {
                        let addr_octs = addr.addr.octets();
                        encoder
                            .array(2)
                            .unwrap()
                            .tag(minicbor::data::Tag::new(20))
                            .unwrap()
                            .str(iface_id)
                            .unwrap()
                            .tag(minicbor::data::Tag::new(54))
                            .unwrap()
                            .bytes(&addr_octs)
                            .unwrap();
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Patch);
                        request.set_path(&self.location);
                        request
                            .message
                            .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
                        request.message.set_payload(&buffer).unwrap();
                        request
                    }
                    IfconfigOperation::Up => {
                        encoder
                            .array(2)
                            .unwrap()
                            .tag(minicbor::data::Tag::new(20))
                            .unwrap()
                            .str(iface_id)
                            .unwrap()
                            .tag(minicbor::data::Tag::new(303))
                            .unwrap()
                            .bool(true)
                            .unwrap();
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Patch);
                        request.set_path(&self.location);
                        request
                            .message
                            .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
                        request.message.set_payload(&buffer).unwrap();
                        request
                    }
                    IfconfigOperation::Down => {
                        encoder
                            .array(2)
                            .unwrap()
                            .tag(minicbor::data::Tag::new(20))
                            .unwrap()
                            .str(iface_id)
                            .unwrap()
                            .tag(minicbor::data::Tag::new(303))
                            .unwrap()
                            .bool(false)
                            .unwrap();
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Patch);
                        request.set_path(&self.location);
                        request
                            .message
                            .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
                        request.message.set_payload(&buffer).unwrap();
                        request
                    }
                    IfconfigOperation::Set { key: _, value: _ } => todo!(),
                }
            } else {
                // Could query multiple iface here but the legacy command doesn't do this
                // so we don't either (but we could)
                encoder.array(1).unwrap();
                encoder.str(iface_id).unwrap();
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
        } else {
            let mut request: CoapRequest<String> = CoapRequest::new();
            request.set_method(Method::Get);
            request.set_path(&self.location);
            request
        };

        request
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        self.payload.clone_from(&response.payload);
        let mut out = String::new();

        if let Some(operation) = &self.cli.operation {
            match operation {
                IfconfigOperation::Add { addr: _ } | IfconfigOperation::Del { addr: _ } => {
                    let resp_status = match response.header.code {
                        coap_lite::MessageClass::Response(ref code) => code,
                        _ => &coap_lite::ResponseType::UnKnown,
                    };
                    if resp_status.is_error() {
                        let _ = writeln!(out, "Couldn't add/del ip address");
                    }
                }
                _ => (), // no op
            }
        } else {
            out = decode_netif_list_into_string(&self.payload);
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

struct Mac {
    data: [u8; 8],
}

impl Mac {
    fn new(data: &[u8; 8]) -> Self {
        Self { data: *data }
    }
}

impl std::fmt::Display for Mac {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let last_i = self.data.len() - 1;
        for i in 0..last_i {
            write!(out, "{:02X}:", self.data[i])?;
        }
        write!(out, "{:02X}", self.data[last_i])
    }
}

#[derive(Clone, Debug)]
struct Ipv6AddrCidr {
    addr: std::net::Ipv6Addr,
    prefix: u8,
}

impl std::fmt::Display for Ipv6AddrCidr {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if self.addr.is_multicast() {
            write!(out, "{}", self.addr)
        } else {
            write!(out, "{}/{}", self.addr, self.prefix)
        }
    }
}

impl Ipv6AddrCidr {
    fn from_cbor(decoder: &mut Decoder) -> Self {
        // rfc9164#section-3.1.1 "Address Format"
        if decoder.probe().bytes().is_ok() {
            // rfc9164#name-ipv6 "to be encoded as a sixteen-byte byte string"
            let data: &[u8; 16] = decoder.bytes().unwrap().try_into().unwrap();
            let addr = std::net::Ipv6Addr::from(*data);
            Self {
                addr,
                prefix: if addr.is_unicast_link_local() {
                    64
                } else {
                    128
                },
            }
        } else if decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            // rfc9164#section-3.1.3 "Interface Definition"
            let (addr, prefix) = if decoder.probe().bytes().is_ok() {
                let data: &[u8; 16] = decoder.bytes().unwrap().try_into().unwrap();
                let addr = std::net::Ipv6Addr::from(*data);
                let prefix = decoder.u8().unwrap_or(128);
                (addr, prefix)
            } else
            // rfc9164#section-3.1.2 "Prefix Format"
            if decoder.probe().u8().is_ok() {
                let prefix = decoder.u8().unwrap_or(128);
                let data: &[u8; 16] = decoder.bytes().unwrap().try_into().unwrap();
                (std::net::Ipv6Addr::from(*data), prefix)
            } else {
                panic!();
            };
            Self { addr, prefix }
        } else {
            panic!();
        }
    }
}

impl std::str::FromStr for Ipv6AddrCidr {
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

struct Iface {
    name: String,
    mac: Option<Mac>,
    ipv6addr: Vec<Ipv6AddrCidr>,
    wired: bool,
}

impl Iface {
    fn from_cbor(decoder: &mut Decoder) -> Self {
        let mut me = Self {
            name: "NoName".to_string(),
            mac: None,
            ipv6addr: vec![],
            wired: true,
        };

        while decoder.probe().tag().is_ok() {
            match decoder.tag().unwrap().as_u64() {
                20 => {
                    me.name = decoder.str().unwrap().to_string();
                }
                48 => {
                    me.mac = Some(Mac::new(
                        decoder
                            .bytes()
                            .unwrap()
                            .try_into()
                            .expect("Mac should be 8 bytes"),
                    ));
                }
                54 => {
                    me.ipv6addr.push(Ipv6AddrCidr::from_cbor(decoder));
                }
                302 => {
                    me.wired = decoder.bool().unwrap();
                }
                _ => continue,
            }
        }
        me
    }
}

impl std::fmt::Display for Iface {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let _ = writeln!(out, "Iface {}", self.name);
        if let Some(mac) = &self.mac {
            writeln!(out, "   HWaddr: {mac}")?;
        }
        if self.wired {
            writeln!(out, "   Link type: wired")?;
        } else {
            writeln!(out, "   Link type: wireless")?;
        }
        for ip in &self.ipv6addr {
            if ip.addr.is_multicast() {
                writeln!(out, "   group: {ip}")?;
            } else {
                writeln!(out, "   addr: {ip}")?;
            }
        }
        Ok(())
    }
}

fn decode_netif_list_into_string(data: &[u8]) -> String {
    let mut out = String::new();
    let mut decoder = Decoder::new(data);

    decoder.array().unwrap();
    {
        while decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            let _ = writeln!(out, "{}", Iface::from_cbor(&mut decoder));
            if let Ok(minicbor::data::Type::Break) = decoder.probe().datatype() {
                decoder.skip().unwrap();
            }
        }
    }
    out
}
