use std::fmt::Write;

use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

use super::Command;
use super::CommandHandler;
use super::CommandRegistry;
use super::HandlerType;

/// Explicit example for a command that queries the well-known/core. Could have been done
/// using the template command, but explicit for demonstration.
pub struct Wkc {
    location: String,
    buffer: String,
    finished: bool,
    displayable: bool,
}

impl CommandRegistry for Wkc {
    fn cmd() -> Command {
        Command {
            cmd: "Wkc".to_owned(),
            description: "Query the wkc".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/.well-known/core".to_owned()],
        }
    }

    fn parse(cmd: &Command, _args: String) -> Result<HandlerType, String> {
        Ok(HandlerType::Configuration(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            finished: false,
            displayable: false,
        })))
    }
}

impl CommandHandler for Wkc {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(&self.location);
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
