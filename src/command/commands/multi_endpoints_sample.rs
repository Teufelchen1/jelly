use std::fmt::Write;

use coap_lite::RequestType as Method;
use coap_lite::{CoapRequest, Packet};

use super::Command;
use super::CommandHandler;
use super::CommandType;

/// This is an example for writing a command that needs to issue multiple CoAP requests and
/// keep track of the state while doing so.
struct MultiEndpointSample {
    buffer: String,
    finished: bool,
    displayable: bool,
    state_machine: usize,
}

pub fn cmd() -> Command {
    Command {
        cmd: "MultiEndpointSample".to_owned(),
        description: "Query multiple endpoints at once!".to_owned(),
        parse: |c, a| Ok(CommandType::CoAP(parse(c, a))),
        required_endpoints: vec![
            "/jelly/board".to_owned(),
            "/shell/reboot".to_owned(),
            "/.well-known/core".to_owned(),
        ],
    }
}

fn parse(_cmd: &Command, _args: &str) -> Box<dyn CommandHandler> {
    Box::new(MultiEndpointSample {
        buffer: "==== Fetched a lot! ====\n".to_owned(),
        finished: false,
        displayable: false,
        state_machine: 0,
    })
}

impl CommandHandler for MultiEndpointSample {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path("/jelly/board");
        request
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        let decoded = String::from_utf8_lossy(&response.payload);

        match self.state_machine {
            0 => {
                self.buffer += &decoded.replace(',', "\n");
                self.buffer += "\n";

                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path("/shell/reboot");

                self.state_machine += 1;
                Some(request)
            }
            1 => {
                self.buffer += &decoded.replace(',', "\n");
                self.buffer += "\n";

                let mut request: CoapRequest<String> = CoapRequest::new();
                request.set_method(Method::Get);
                request.set_path("/.well-known/core");

                self.state_machine += 1;
                Some(request)
            }
            _ => {
                self.buffer += &decoded.replace(',', "\n");
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
