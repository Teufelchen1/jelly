use super::{Command, CommandHandler, CommandType};

use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_lite::{CoapOption, ContentFormat, option_value::OptionValueString};
use coap_lite::{MessageClass, ResponseType};

/// The over-all state of a the Edit command, which can change over several invocations of it.
///
/// It is contained in a `RefCell` and taken out for single command executions so that the state
/// can be mutated across the steps of a command execution (even though `parse` only gets it
/// through a shared reference), and only borrowed for short operations from the command (and thus
/// to never fail).
///
/// The error variant is what is to be shown in the next stage, not necessarily immediately: For
/// example, a state is crated when GET'ting a resource is not shown, but prepared for if and when
/// the user issues the `edit` command.
type State = std::cell::RefCell<Result<EditableState, String>>;

/// State indicating that there is a resource that can be edited, and which state it was seen in,
/// or should be PUT as.
pub struct EditableState {
    path: Vec<String>,
    content_format: Option<ContentFormat>,
    payload: Vec<u8>,
}

/// Groupings of content formats (because CBOR tend to become many)
enum DecoderFormat {
    Cbor,
    Text,
    Other,
}

/// State of a command instance
pub enum CoapEdit {
    /// The command has been issued and parsed, and (due to when errors may be handled) also has
    /// been run through to the editing process.
    ///
    /// This carries around what is effectively the pointer to where eventually, the edited state
    /// will be returned into the the (non-instance) command.
    Nascent(EditableState, std::rc::Rc<dyn std::any::Any>),
    /// PUT has been sent; waiting for response. If PUT comes back positive, the internal state
    /// can be written back for follow-up edits.
    ///
    /// This carries around what is effectively the pointer to where eventually, the edited state
    /// will be returned into the the (non-instance) command.
    RunningPut(EditableState, std::rc::Rc<dyn std::any::Any>),
    /// The command has completed; the editable state has been returned into the `edit` command, so
    /// that follow-up edits can happen.
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
                    .to_owned(),
            );
        }
        if request.message.options().any(|o| o.0 != &11) {
            return Err("Can not edit after request that had unknown options.".to_owned());
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
            .unwrap_or_default()
            .into_iter()
            .map(|res| res.map(|s| s.0))
            .collect();
        let path = path.map_err(|_| "Unprocessable Uri-Path option in request".to_owned())?;

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
        }

        let cmd_state = cmd
            .state
            .as_ref()
            .expect("edit is always created with state");
        let mut state = (*cmd_state)
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
            Err("edit failed: Previous edit did not complete yet.\n".to_owned()),
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

        let content_format = match content_format {
            ContentFormat::ApplicationCBOR | ContentFormat::ApplicationCborSeq => {
                DecoderFormat::Cbor
            }
            ContentFormat::TextPlain => DecoderFormat::Text,
            _ => DecoderFormat::Other,
        };

        let decoded = match content_format {
            DecoderFormat::Cbor => {
                let parsed = cbor_edn::Sequence::from_cbor(&working_state.payload)
                    .map_err(|e| format!("parsing error, can not edit: {e}"))?;
                parsed.serialize()
            }
            DecoderFormat::Text => str::from_utf8(&working_state.payload)
                .map(std::borrow::ToOwned::to_owned)
                .map_err(|e| format!("plain text needs to be UTF-8 encoded ({e})"))?,
            DecoderFormat::Other => hex::encode(&working_state.payload),
        };

        let tempfile_suffix = working_state.path.last().map_or("root", |s| s);
        let edited = match crate::tui::hands_off_terminal(|| {
            edit::edit_with_builder(
                decoded,
                edit::Builder::new().suffix("-").suffix(tempfile_suffix),
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
            DecoderFormat::Cbor => {
                let parsed = cbor_edn::Sequence::parse(&edited)
                    .map_err(|e| format!("can not convert to CBOR due to parsing error: {e}"))?;
                parsed.to_cbor().map_err(|e| {
                    format!("can not convert to CBOR due to unrecognized parts: {e}")
                })?
            }
            DecoderFormat::Text => edited.as_bytes().into(),
            DecoderFormat::Other => {
                // It'd be nice and efficient to have an option in hex to just ignore space, but
                // this will do.
                let replaced = edited.replace([' ', '\n'], "");
                hex::decode(replaced)
                    .map_err(|e| format!("can not convert hex back to binary: {e}"))?
            }
        };

        // FIXME: It would be nice if we could store the edited text, eg. for an `edit --recover` command

        Ok(CommandType::CoAP(Box::new(Self::Nascent(
            working_state,
            cmd_state.clone(),
        ))))
    }
}

impl CommandHandler for CoapEdit {
    #[expect(
        clippy::assigning_clones,
        reason = "clippy doesn't see that this is initialized empty and thus without a usable allocation"
    )]
    fn init(&mut self) -> CoapRequest<String> {
        let Self::Nascent(state, cmd_state) = core::mem::replace(
            self,
            Self::Error("bug: init() returned without setting a new self".to_owned()),
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

        *self = Self::RunningPut(state, cmd_state);

        request
    }

    fn handle(&mut self, payload: &coap_lite::Packet) -> Option<coap_lite::CoapRequest<String>> {
        let Self::RunningPut(state_if_successful, cmd_state) = core::mem::replace(
            self,
            Self::Error("bug: handle() returned without setting a new self".to_owned()),
        ) else {
            unreachable!("response handling only happens after entering the RunningPut state");
        };

        if payload.header.code == MessageClass::Response(ResponseType::Changed) {
            *cmd_state
                .downcast_ref::<State>()
                .expect("edit always has a State as payload")
                .borrow_mut() = Ok(state_if_successful);
            *self = Self::Successful;
        } else {
            *self = Self::Error(format!(
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
        matches!(self, Self::Successful | Self::Error(_))
    }

    fn display(&self, buffer: &mut String) {
        use std::fmt::Write;
        match &self {
            Self::Successful => writeln!(buffer, "Edit was applied successfullly.\n").unwrap(),
            Self::Error(e) => writeln!(buffer, "Editing failed: {e}.\n").unwrap(),
            _ => unreachable!("we didn't ask to be displayed yet"),
        }
    }
}
