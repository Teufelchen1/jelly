use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

use crate::commands::Command;
use crate::commands::CommandHandler;
use crate::commands::CommandRegistry;

/// This is a template command. It shows the minimum setup for making a
/// CoAP GET request to a single endpoint.
/// This is used make all endpoints in the /.well-known/core available for a
/// quick GET request via autocomplete.
pub struct CoapGet {
    path: String,
    buffer: Vec<u8>,
}

/// Interface with the library and handler
impl CommandRegistry for CoapGet {
    fn cmd() -> Command {
        Command {
            cmd: "CoapGet".to_owned(),
            description: "GET a CoAP resource".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec![],
        }
    }

    // Saves the first path of this command...so this won't work with commands that need multiple.
    fn parse(cmd: &Command, _args: String) -> Result<Box<dyn CommandHandler>, String> {
        Ok(Box::new(Self {
            path: cmd.required_endpoints[0].clone(),
            buffer: vec![],
        }))
    }
}

impl CommandHandler for CoapGet {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        // The saved path is needed here to generate the coap request
        request.set_path(&self.path);
        request
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        self.buffer.extend_from_slice(payload);
        None
    }

    /// Asks the handler if it wants to display anything. Usually called after a response was
    /// processed.
    fn want_display(&self) -> bool {
        true
    }

    /// Provides a buffer into which the handler can write the result.
    fn display(&self, buffer: &mut String) {
        buffer.push_str(&String::from_utf8_lossy(&self.buffer));
    }
}
