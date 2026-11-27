use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_message::MinimalWritableMessage;

use super::Command;
use super::CommandHandler;

/// This is a template command. It shows the minimum setup for making a
/// CoAP GET request to a single endpoint. The result is not displayed to the user.
/// Allthought the default overview of received CoAP responses will show a summary.
/// This is used to make all endpoints in the /.well-known/core available for a
/// quick get request via autocomplete.
pub struct CoapGet(String, Option<String>);

impl CoapGet {
    // Saves the first path of this command...so this won't work with commands that need multiple.
    pub fn parse(cmd: &Command, _args: &str) -> Box<dyn CommandHandler> {
        let (path, query) = match cmd.cmd.split_once('&') {
            Some((path, query)) => {
                if query != "" {
                    (path.to_owned(), Some(query.to_owned()))
                } else {
                    (path.to_owned(), None)
                }
            }
            None => (cmd.required_endpoints[0].to_owned(), None),
        };
        Box::new(Self(path, query))
    }
}

impl CommandHandler for CoapGet {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);

        if let Some(query) = &self.1 {
            let _ = request
                .message
                .add_option_str(coap_lite::CoapOption::UriQuery, &query);
        }

        // Might also be a bug in coap-lite that "/" should be turned into an
        // empty option set; documentation isn't quite conclusive.
        if self.0 != "/" {
            println!("Setting path: {:?}", self.0);
            // The saved path is needed here to generate the coap request
            request.set_path(&self.0);
        }
        request
    }
}
