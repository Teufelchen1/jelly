use std::fmt::Write;

use coap_lite::RequestType as Method;
use coap_lite::{CoapRequest, Packet};

use super::Command;
use super::CommandHandler;

/// Explicit example for a command that queries the well-known/core. Could have been done
/// using the template command, but explicit for demonstration.
pub struct Wkc {
    location: String,
    buffer: String,
    finished: bool,
    displayable: bool,
}

impl Wkc {
    pub fn cmd() -> Command {
        Command {
            cmd: "Wkc".to_owned(),
            description: "Query the wkc".to_owned(),
            parse: |c, a| Ok(Self::parse(c, a)),
            required_endpoints: vec!["/.well-known/core".to_owned()],
        }
    }

    fn parse(cmd: &Command, _args: &str) -> Box<dyn CommandHandler> {
        Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            finished: false,
            displayable: false,
        })
    }
}

impl CommandHandler for Wkc {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(&self.location);
        request
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        self.buffer = String::from_utf8_lossy(&response.payload).to_string();
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
