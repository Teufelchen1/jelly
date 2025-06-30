use std::fmt::Write;

use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

use crate::commands::Command;
use crate::commands::CommandHandler;
use crate::commands::CommandRegistry;

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
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path("/riot/board");
        request
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
