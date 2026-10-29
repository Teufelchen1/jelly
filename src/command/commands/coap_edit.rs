use super::{Command, CommandHandler, CommandType};

use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_lite::{CoapOption, ContentFormat, option_value::OptionValueString};
use coap_lite::{MessageClass, ResponseType};

/// The over-all state of a the Edit command, which can change over several invocations of it.
///
/// It is contained in a RefCell and taken out for single command executions, and only borrowed for
/// short operations from the command (and thus to never fail).
type State = std::cell::RefCell<Result<EditableState, String>>;
/// State indicating that there is a resource that can be edited, and which state it is in.
pub struct EditableState {
    path: Vec<String>,
    content_format: Option<ContentFormat>,
    payload: Vec<u8>,
}

pub enum CoapEdit {
    Nascent(EditableState, std::rc::Rc<dyn std::any::Any>),
    // PUT has been sent; waiting for response. If PUT comes back positive, the internal state
    // can be written back for follow-up edits.
    RunningPut(EditableState, std::rc::Rc<dyn std::any::Any>),
    Successful,
    Error(String),
}

impl CoapEdit {
    // See Command::new_edit for arguments and error handling rationale
    pub fn state_for(
        request: &CoapRequest<String>,
        response: &coap_lite::Packet,
    ) -> Result<EditableState, String> {
        if request.get_method() != &Method::Get {
            return Err(
                "Can not edit non-GET request (bug: should have been caught at new_edit caller)"
                    .to_string(),
            );
        }
        if request.message.options().any(|o| o.0 != &11) {
            return Err("Can not edit after request that had unknown options.".to_string());
        }

        if response.header.code != MessageClass::Response(ResponseType::Content) {
            // Technically this could be a successful state that is not Content, but we're not
            // requesting an ETag, not creating a new resource etc.
            return Err(format!(
                "GETting the data from {} was not successful, response code {}",
                request
                    .source
                    .as_ref()
                    .map_or_else(|| "(unknown source)", |s| s.as_str()),
                response.header.code
            ));
        }

        let content_format = response.get_content_format();

        let path: Result<Vec<_>, _> = request
            .message
            .get_options_as::<OptionValueString>(CoapOption::UriPath)
            .unwrap_or(Default::default())
            .into_iter()
            .map(|res| res.map(|s| s.0.to_owned()))
            .collect();
        let path = path.map_err(|_| "Unprocessable Uri-Path option in request".to_string())?;

        // FIXME: carry ETag, send If-Match
        // FIXME: check for critical response options; store content format and etag

        Ok(EditableState {
            path,
            content_format,
            payload: response.payload.clone(),
        })
    }

    pub fn parse(cmd: &Command, args: &str) -> Result<CommandType, String> {
        if args != "edit" {
            return Err("usage: `edit` (invoked after any CoAP GET request)\n".to_owned());
        };

        let cmd_state = cmd
            .state
            .as_ref()
            .expect("edit is always created with state");
        let mut state = cmd_state
            .downcast_ref::<State>()
            .expect("edit always has a State as payload")
            .borrow_mut();

        if let Err(e) = &*state {
            return Err(e.clone());
        }

        // Leaving a helpful note to the next invocation, until we fill a state back in on
        // completion.
        let working_state = core::mem::replace(
            &mut *state,
            Err("edit failed: Previous edit did not complete yet.\n".to_string()),
        );
        // checked above, but we didn't want to spoil the error message by taking it out up there.
        let mut working_state = working_state.unwrap();

        // We have to edit right away, for at .parse() time is the last time before sending the
        // request that we can actually decide that things did *not* work.

        let content_format = working_state.content_format.unwrap_or_else(|| {
            // Simple heuristic for guessing
            if str::from_utf8(&working_state.payload).is_ok() {
                ContentFormat::TextPlain
            } else {
                ContentFormat::ApplicationOctetStream
            }
        });

        let decoded = match content_format {
            ContentFormat::ApplicationCBOR | ContentFormat::ApplicationCborSeq => {
                let parsed = cbor_edn::Sequence::from_cbor(&working_state.payload)
                    .map_err(|e| format!("parsing error, can not edit: {e}"))?;
                parsed.serialize()
            }
            ContentFormat::TextPlain => str::from_utf8(&working_state.payload)
                .map(|s| s.to_owned())
                .map_err(|e| format!("plain text needs to be UTF-8 encoded ({e})"))?,
            _ => todo!(),
        };

        let tempfile_suffix = working_state.path.last().map_or("root", |s| s);
        let edited = match crate::tui::hands_off_terminal(|| {
            edit::edit_with_builder(
                decoded,
                &edit::Builder::new().suffix("-").suffix(tempfile_suffix),
            )
            .map_err(|e| format!("editor error: {e}"))
        }) {
            Ok(edited) => edited,
            Err(e) => {
                // We haven't updated state so we can just put back hat we had
                *state = Ok(working_state);
                return Err(e);
            }
        };

        working_state.payload = match content_format {
            ContentFormat::ApplicationCBOR | ContentFormat::ApplicationCborSeq => {
                let parsed = cbor_edn::Sequence::parse(&edited)
                    .map_err(|e| format!("can not convert to CBOR due to parsing error: {e}"))?;
                parsed.to_cbor().map_err(|e| {
                    format!("can not convert to CBOR due to unrecognized parts: {e}")
                })?
            }
            ContentFormat::TextPlain => edited.as_bytes().into(),
            _ => todo!(),
        };

        Ok(CommandType::CoAP(Box::new(Self::Nascent(
            working_state,
            cmd_state.clone(),
        ))))
    }
}

impl CommandHandler for CoapEdit {
    fn init(&mut self) -> CoapRequest<String> {
        let CoapEdit::Nascent(state, cmd_state) = core::mem::replace(
            self,
            CoapEdit::Error("bug: init() returned without setting a new self".to_string()),
        ) else {
            unreachable!(
                "init is called once per parsed CoapEdit, which is constructed in Nascent state"
            );
        };

        let mut request = CoapRequest::new();
        request.set_method(Method::Put);
        for p in &state.path {
            request
                .message
                .add_option(CoapOption::UriPath, p.as_bytes().to_vec());
        }
        request.message.payload = state.payload.clone();

        *self = CoapEdit::RunningPut(
            EditableState {
                path: state.path,
                content_format: state.content_format,
                payload: state.payload,
            },
            cmd_state,
        );

        request
    }

    fn handle(&mut self, payload: &coap_lite::Packet) -> Option<coap_lite::CoapRequest<String>> {
        let CoapEdit::RunningPut(state_if_successful, cmd_state) = core::mem::replace(
            self,
            CoapEdit::Error("bug: handle() returned without setting a new self".to_string()),
        ) else {
            unreachable!("response handling only happens after entering the RunningPut state");
        };

        if payload.header.code == MessageClass::Response(ResponseType::Changed) {
            *cmd_state
                .downcast_ref::<State>()
                .expect("edit always has a State as payload")
                .borrow_mut() = Ok(state_if_successful);
            *self = CoapEdit::Successful;
        } else {
            *self = CoapEdit::Error(format!(
                "PUTting the data was not successful, response code {}",
                payload.header.code
            ));
        }

        None
    }

    fn want_display(&self) -> bool {
        self.is_finished()
    }

    fn is_finished(&self) -> bool {
        matches!(self, CoapEdit::Successful | CoapEdit::Error(_))
    }

    fn display(&self, buffer: &mut String) {
        use std::fmt::Write;
        match &self {
            CoapEdit::Successful => writeln!(buffer, "Edit was applied successfullly.\n").unwrap(),
            CoapEdit::Error(e) => writeln!(buffer, "Editing failed: {e}.\n").unwrap(),
            _ => unreachable!("we didn't ask to be displayed yet"),
        }
    }
}
