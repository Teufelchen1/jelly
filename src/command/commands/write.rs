//! Generic PUTting
//!
//! Open issues:
//! * We should really GET first to
//!   * know the typical content format (OK that could come from metadata too)
//!   * know the last ETag the user saw (unless they force-push, aeh, force-put)
//!   * For that, we need to be told of every request.
//! * This should be available unconditionally (but, later when we auto-complete in commands, maybe
//!   prefer resources that are annotated to be writable … oh, core-interfaces is still stalled)
//! * Am I doing error handling right?
//! * Maybe this should be more of an interactive edit command?

use super::Command;
use super::CommandHandler;
// use super::CommandRegistry; // Look moo no handler!

use clap::Parser;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

pub fn cmd() -> Command {
    Command {
        cmd: "write".into(),
        description: "Perform a PUT operation on a resource".into(),
        required_endpoints: vec!["/whoami".into()], // FIXME this is cheating
        parse,
    }
}

#[allow(clippy::needless_pass_by_value, reason = "Required callback signature")]
fn parse(_cmd: &Command, args: String) -> Result<crate::command::BoxedCommandHandler, String> {
    #[derive(Parser, Debug)]
    #[command(name = "write")]
    struct WriteArgs {
        path: String,
        value: String,
    }

    let parsed = WriteArgs::try_parse_from(args.split_whitespace()).map_err(|x| x.to_string())?;

    Ok(Box::new(WriteCommand {
        path: parsed.path,
        value: cbor_edn::StandaloneItem::parse(&parsed.value)
            .map_err(|e| format!("Failed to parse CBOR EDN: {e}"))?
            .to_cbor()
            .map_err(|e| format!("Failed to serialize CBOR: {e}"))?,
        error: None.into(),
    }))
}

#[derive(Debug)]
struct WriteCommand {
    path: String,
    value: Vec<u8>,
    // We only get &self in display but the error comes up in handle where we can't print; shoving
    // it through here with interior mutability.
    error: core::cell::RefCell<Option<String>>,
}

impl CommandHandler for WriteCommand {
    fn init(&mut self) -> coap_lite::CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Put);
        request.set_path(&self.path);
        request.message.payload.clone_from(&self.value);
        request
    }

    fn handle(&mut self, response: &coap_lite::Packet) -> Option<CoapRequest<String>> {
        use coap_lite::{MessageClass::Response, ResponseType::Changed};
        if response.header.code != Response(Changed) {
            self.error = Some(format!(
                "Write failed; unexpected response code {}",
                response.header.code
            ))
            .into();
        }
        None
    }

    fn want_display(&self) -> bool {
        self.error.borrow().is_some()
    }

    fn display(&self, buffer: &mut String) {
        use std::fmt::Write;
        writeln!(
            buffer,
            "{}",
            self.error.take().expect("We jus checked in want_display()")
        )
        .unwrap();
    }
}
