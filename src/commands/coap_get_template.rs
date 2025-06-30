use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

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
