use std::fmt::Write;

use clap::Parser;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_message::MinimalWritableMessage;
use minicbor::Encoder;

use crate::commands::Command;
use crate::commands::CommandHandler;
use crate::commands::CommandRegistry;

pub struct CoapGet(String);

impl CommandRegistry for CoapGet {
    fn cmd() -> Command {
        Command {
            cmd: "CoapGet".to_owned(),
            description: "GET a CoAP resource".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec![],
        }
    }

    fn parse(cmd: &Command, _args: String) -> Result<Box<dyn CommandHandler>, String> {
        Ok(Box::new(Self(cmd.required_endpoints[0].clone())))
    }
}

impl CommandHandler for CoapGet {
    fn init(&mut self) -> Option<CoapRequest<String>> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(&self.0);
        Some(request)
    }
}

pub struct Wkc {
    location: String,
    buffer: String,
    finished: bool,
    displayable: bool,
}

impl CommandRegistry for Wkc {
    fn cmd() -> Command {
        Command {
            cmd: "wkc".to_owned(),
            description: "Query the wkc".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/.well-known/core".to_owned()],
        }
    }

    fn parse(cmd: &Command, _args: String) -> Result<Box<dyn CommandHandler>, String> {
        Ok(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            finished: false,
            displayable: false,
        }))
    }
}

impl CommandHandler for Wkc {
    fn init(&mut self) -> Option<CoapRequest<String>> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(&self.location);
        Some(request)
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        self.buffer = String::from_utf8_lossy(payload).to_string();
        self.buffer = self.buffer.replace(',', "\n");
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
}

#[derive(Parser, Debug)]
#[command(name = "SampleCommand")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is an example command")]
pub struct SampleCommandCli {
    #[arg(long)]
    caps: bool,
    #[arg(long, default_value_t = 1)]
    repeats: usize,
}

pub struct SampleCommand {
    location: String,
    buffer: String,
    finished: bool,
    displayable: bool,
    cli: SampleCommandCli,
}

impl CommandRegistry for SampleCommand {
    fn cmd() -> Command {
        Command {
            cmd: "SampleCommand".to_owned(),
            description: "An example coap based command".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/SampleCommand".to_owned()],
        }
    }

    fn parse(cmd: &Command, args: String) -> Result<Box<dyn CommandHandler>, String> {
        let cli =
            SampleCommandCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
        Ok(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            finished: false,
            displayable: false,
            cli,
        }))
    }
}

impl CommandHandler for SampleCommand {
    fn init(&mut self) -> Option<CoapRequest<String>> {
        let mut buffer: [u8; 4] = [0; 4];
        let mut encoder = Encoder::new(&mut buffer[..]);

        let _ = encoder
            .array(2)
            .unwrap()
            .bool(self.cli.caps)
            .unwrap()
            .u8(self.cli.repeats.try_into().unwrap())
            .unwrap()
            .end();

        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Post);
        request.set_path(&self.location);
        request
            .message
            .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
        request.message.set_payload(&buffer).unwrap();
        Some(request)
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        self.buffer = String::from_utf8_lossy(payload).to_string();
        self.buffer = self.buffer.replace(',', "\n");
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
}

pub struct MultiEndpointSample {
    buffer: String,
    finished: bool,
    displayable: bool,
    state_machine: usize,
}

impl CommandRegistry for MultiEndpointSample {
    fn cmd() -> Command {
        Command {
            cmd: "MultiEndpointSample".to_owned(),
            description: "Query multiple endpoints at once!".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec![
                "/riot/board".to_owned(),
                "/shell/reboot".to_owned(),
                "/.well-known/core".to_owned(),
            ],
        }
    }

    fn parse(_cmd: &Command, _args: String) -> Result<Box<dyn CommandHandler>, String> {
        Ok(Box::new(Self {
            buffer: "==== Fetched a lot! ====\n".to_owned(),
            finished: false,
            displayable: false,
            state_machine: 0,
        }))
    }
}

impl CommandHandler for MultiEndpointSample {
    fn init(&mut self) -> Option<CoapRequest<String>> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path("/riot/board");
        Some(request)
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        match self.state_machine {
            0 => {
                self.buffer += &String::from_utf8_lossy(payload)
                    .to_string()
                    .replace(',', "\n");
                self.buffer += "\n";

                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path("/shell/reboot");

                self.state_machine += 1;
                Some(request)
            }
            1 => {
                self.buffer += &String::from_utf8_lossy(payload)
                    .to_string()
                    .replace(',', "\n");
                self.buffer += "\n";

                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path("/.well-known/core");

                self.state_machine += 1;
                Some(request)
            }
            _ => {
                self.buffer += &String::from_utf8_lossy(payload)
                    .to_string()
                    .replace(',', "\n");
                self.buffer += "\n";

                self.buffer += "==== Done! ====\n";

                self.finished = true;
                self.displayable = true;
                None
            }
        }
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
}
