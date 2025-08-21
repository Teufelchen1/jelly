use std::fmt::Write;

use clap::Parser;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_message::MinimalWritableMessage;
use minicbor::Encoder;

use super::Command;
use super::CommandHandler;
use super::CommandRegistry;
use super::HandlerType;

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

    fn parse(cmd: &Command, args: String) -> Result<HandlerType, String> {
        let cli =
            SampleCommandCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
        Ok(HandlerType::Configuration(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            finished: false,
            displayable: false,
            cli,
        })))
    }
}

impl CommandHandler for SampleCommand {
    fn init(&mut self) -> CoapRequest<String> {
        let mut buffer: [u8; 4] = [0; 4];
        let mut encoder = Encoder::new(&mut buffer[..]);

        encoder
            .array(2)
            .unwrap()
            .bool(self.cli.caps)
            .unwrap()
            .u8(self.cli.repeats.try_into().unwrap())
            .unwrap()
            .end()
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
