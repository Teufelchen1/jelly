use coap_lite::RequestType as Method;
use coap_lite::{CoapRequest, Packet};

use super::{Command, CommandHandler, CommandType};

struct ToggleLed0 {
    location: String,
    buffer: String,
    finished: bool,
}

pub fn cmd() -> Command {
    Command {
        cmd: "ToggleLed0".to_owned(),
        description: "Toggles LED0".to_owned(),
        parse: parse_cli,
        required_endpoints: vec!["/jelly/led0".to_owned()],
    }
}

fn parse_cli(cmd: &Command, _args: &str) -> Result<CommandType, String> {
    Ok(CommandType::CoAP(Box::new(ToggleLed0 {
        location: cmd.required_endpoints[0].clone(),
        buffer: String::new(),
        finished: false,
    })))
}

impl CommandHandler for ToggleLed0 {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(&self.location);
        request
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        let resp_status = match response.header.code {
            coap_lite::MessageClass::Response(ref code) => code,
            _ => &coap_lite::ResponseType::UnKnown,
        };
        if resp_status.is_error() {
            self.buffer.push_str("An error occured!");
        } else {
            self.buffer.push_str("Toggled the LED0!");
        }
        self.finished = true;
        None
    }

    fn want_display(&self) -> bool {
        true
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn display(&self, buffer: &mut String) {
        buffer.push_str(&self.buffer);
    }
}
