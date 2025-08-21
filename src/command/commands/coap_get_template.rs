use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

use super::Command;
use super::CommandHandler;
use super::CommandRegistry;
use super::HandlerType;

/// This is a template command. It shows the minimum setup for making a
/// CoAP GET request to a single endpoint. The result is not displayed to the user.
/// Allthought the default overview of received CoAP responses will show a summary.
/// This is used make all endpoints in the /.well-known/core available for a
/// quick get request via autocomplete.
pub struct CoapGet(String);

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
    fn parse(cmd: &Command, _args: String) -> Result<HandlerType, String> {
        let new_self = Self(cmd.required_endpoints[0].clone());
        Ok(HandlerType::Configuration(Box::new(new_self)))
    }
}

impl CommandHandler for CoapGet {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        // The saved path is needed here to generate the coap request
        request.set_path(&self.0);
        request
    }
}
