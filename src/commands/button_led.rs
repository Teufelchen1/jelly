use std::fmt::Write;

use clap::Parser;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_message::MinimalWritableMessage;
use minicbor::Decoder;
use minicbor::Encoder;

use crate::commands::Command;
use crate::commands::CommandHandler;
use crate::commands::CommandRegistry;

/// This is an example on how to use cbor as payload for the coap request.
#[derive(Parser, Debug)]
#[command(name = "Pressure")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "Set LEDs based on button presses")]
pub struct ButtonLedCli {}

pub struct ButtonLed {
    location: String,
    payload: [bool; 4],
    finished: bool,
    displayable: bool,
    state: u8,
}

impl CommandRegistry for ButtonLed {
    fn cmd() -> Command {
        Command {
            cmd: "Pressure".to_owned(),
            description: "Set LEDs based on button presses".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/Saul".to_owned()],
        }
    }

    fn parse(cmd: &Command, args: String) -> Result<Box<dyn CommandHandler>, String> {
        ButtonLedCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
        Ok(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            payload: [false, false, false, false],
            finished: false,
            displayable: false,
            state: 0,
        }))
    }
}

impl ButtonLed {
    fn read_button(id: u8) -> [u8; 12] {
        let mut buffer: [u8; 12] = [0; 12];
        let mut encoder = Encoder::new(&mut buffer[..]);

        encoder
            .array(2)
            .unwrap()
            .u8(1) // Read
            .unwrap()
            .u8(id) // Button id
            .unwrap()
            .end()
            .unwrap();
        buffer
    }

    fn write_led(id: u8, on: bool) -> [u8; 12] {
        let mut buffer: [u8; 12] = [0; 12];
        let mut encoder = Encoder::new(&mut buffer[..]);
        let value = if on { 255 } else { 0 };

        encoder
            .array(3)
            .unwrap()
            .u8(2) // Write
            .unwrap()
            .u8(id) // Led id
            .unwrap()
            .u8(value) // Strength
            .unwrap()
            .end()
            .unwrap();
        buffer
    }

    fn coap_request(&self, payload: &[u8]) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Post);
        request.set_path(&self.location);
        request
            .message
            .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
        request.message.set_payload(payload).unwrap();
        request
    }
}

impl CommandHandler for ButtonLed {
    fn init(&mut self) -> CoapRequest<String> {
        let cbor = Self::read_button(0);
        self.state += 1;
        self.coap_request(&cbor)
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        let mut decoder = Decoder::new(payload);
        let query = match self.state {
            1..4 => {
                let _data = decoder.map();
                decoder.skip().unwrap();
                let _name = decoder.str().unwrap();
                decoder.skip().unwrap();
                let value = decoder.bool().unwrap();
                self.payload[(self.state - 1) as usize] = value;
                let cbor = Self::read_button(self.state);
                Some(self.coap_request(&cbor))
            }
            4 => {
                let _data = decoder.map();
                decoder.skip().unwrap();
                let _name = decoder.str().unwrap();
                decoder.skip().unwrap();
                let value = decoder.bool().unwrap();
                self.payload[(self.state - 1) as usize] = value;
                let cbor = Self::write_led(self.state, self.payload[(self.state - 4) as usize]);
                Some(self.coap_request(&cbor))
            }
            5..8 => {
                let cbor = Self::write_led(self.state, self.payload[(self.state - 4) as usize]);
                Some(self.coap_request(&cbor))
            }
            _ => None,
        };
        self.state += 1;
        if query.is_none() {
            self.displayable = true;
            self.finished = true;
        }
        query
    }

    fn want_display(&self) -> bool {
        self.displayable
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn display(&self, buffer: &mut String) {
        for (i, v) in self.payload.iter().enumerate() {
            if *v {
                let _ = writeln!(buffer, "Set LED{i} to ON");
            } else {
                let _ = writeln!(buffer, "Set LED{i} to OFF");
            }
        }
    }
}
