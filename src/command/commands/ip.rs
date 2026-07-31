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
        #[arg(required = true, num_args = 1.., value_parser = clap::value_parser!(u8), value_delimiter = ' ')]
        interface_names: Vec<u8>,
    },
    /// Write a 8 bit value into an actuator
    Write { id: u8, data: u8 },
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
        let mut buffer: [u8; 12] = [0; 12];
        let mut encoder = Encoder::new(&mut buffer[..]);

        let request = match &self.cli.operation {
            None | Some(IpOperation::List) => {
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path(&self.location);
                request
            }
            Some(IpOperation::Read { interface_names }) => {
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path(&self.location);
                request
                // encoder
                //     .array(interface_names.len().try_into().unwrap())
                //     .unwrap();
                // for id in interface_names {
                //     encoder.u8(*id).unwrap();
                // }

                // encoder.end().unwrap();
                // let mut request: CoapRequest<String> = CoapRequest::new();
                // request.set_method(Method::Get);
                // request.set_path(&self.location);
                // request
                //     .message
                //     .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
                // request.message.set_payload(&buffer).unwrap();
                // request
            }
            Some(IpOperation::Write { id, data }) => {
                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path(&self.location);
                request
                // encoder
                //     .array(2)
                //     .unwrap()
                //     .u8(*id)
                //     .unwrap()
                //     .u8(*data)
                //     .unwrap()
                //     .end()
                //     .unwrap();
                // let mut request: CoapRequest<String> = CoapRequest::new();
                // request.set_method(Method::Post);
                // request.set_path(&self.location);
                // request
                //     .message
                //     .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
                // request.message.set_payload(&buffer).unwrap();
                // request
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
                // match senml::pack::Pack::from_cbor(&self.payload) {
                //     Ok(parsed) => {
                //         let _ = writeln!(out, "{:}", parsed.normalize());
                //     }
                //     Err(e) => {
                //         let _ = writeln!(out, "Koens SenML Says:\n{e:?}");
                //     }
                // }
            }
            Some(IpOperation::Write { id: _, data: _ }) => {
                // match senml::record::RawRecord::from_cbor(&self.payload) {
                //     Ok(parsed) => {
                //         let _ = writeln!(out, "Koens SenML Says:\n{parsed:?}");
                //     }
                //     Err(e) => {
                //         let _ = writeln!(out, "Koens SenML Says:\n{e:?}");
                //     }
                // }
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

fn decode_netif_list_into_string(data: &[u8]) -> String {
    fn print_mac(out: &mut String, data: &[u8]) {
        let last_i = data.len() - 1;
        for i in 0..last_i {
            write!(out, "{:02X}:", data[i]).unwrap();
        }
        write!(out, "{:02X}", data[last_i]).unwrap();
    }
    let mut out = String::new();
    let mut decoder = Decoder::new(data);
    decoder.array().unwrap();
    {
        while decoder.probe().array().is_ok() {
            decoder.array().unwrap();
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
                    302 => {
                        if decoder.bool().unwrap() {
                            write!(out, "   Link type: wired").unwrap();
                        } else {
                            write!(out, "   Link type: wireless").unwrap();
                        }
                    }
                    _ => continue,
                }
            }
        }
    }
    out
}
