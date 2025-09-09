use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

use super::Command;
use super::CommandHandler;

/// This is a template command. It shows the minimum setup for making a
/// CoAP GET request to a single endpoint. The result is not displayed to the user.
/// Allthought the default overview of received CoAP responses will show a summary.
/// This is used make all endpoints in the /.well-known/core available for a
/// quick get request via autocomplete.
pub struct CoapGet(String);

impl CoapGet {
    // Saves the first path of this command...so this won't work with commands that need multiple.
    pub fn parse(cmd: &Command, _args: &str) -> Box<dyn CommandHandler> {
        Box::new(Self(cmd.required_endpoints[0].clone()))
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
