use std::fmt::Write;

use clap::Parser;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_message::MinimalWritableMessage;
use minicbor::Decoder;
use minicbor::Encoder;

use super::Command;
use super::CommandHandler;
use super::CommandRegistry;

/// Taken and modified from Alex Martens's clap-num
/// <https://github.com/newAM/clap-num/blob/c6f1065f87f319098943aae75412a0c38c85f11c/src/lib.rs#L347-L384>
///
/// MIT License
///
/// Copyright (c) 2020 - present Alex Martens
///
/// Permission is hereby granted, free of charge, to any person obtaining a copy
/// of this software and associated documentation files (the "Software"), to deal
/// in the Software without restriction, including without limitation the rights
/// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
/// copies of the Software, and to permit persons to whom the Software is
/// furnished to do so, subject to the following conditions:
///
/// The above copyright notice and this permission notice shall be included in all
/// copies or substantial portions of the Software.
///
/// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
/// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
/// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
/// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
/// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
/// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
/// SOFTWARE.
fn maybe_hex(s: &str) -> Result<u32, String> {
    const HEX_PREFIX: &str = "0x";
    const HEX_PREFIX_UPPER: &str = "0X";
    const HEX_PREFIX_LEN: usize = HEX_PREFIX.len();

    let result = if s.starts_with(HEX_PREFIX) || s.starts_with(HEX_PREFIX_UPPER) {
        u32::from_str_radix(&s[HEX_PREFIX_LEN..], 16)
    } else {
        s.parse()
    };

    result.map_err(|e| format!("{e}"))
}

#[derive(Parser, Debug)]
#[command(name = "MemoryRead")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This reads arbitrary memory")]
pub struct MemReadCli {
    #[arg(value_parser=maybe_hex)]
    addr: u32,
    #[arg()]
    size: u32,
}

pub struct MemRead {
    buffer: Vec<u8>,
    finished: bool,
    displayable: bool,
    cli: MemReadCli,
}

impl MemRead {
    fn send_request(&mut self) -> CoapRequest<String> {
        let mut buffer: [u8; 10] = [0; 10];
        let mut encoder = Encoder::new(&mut buffer[..]);

        let num_bytes: u8 = if self.cli.size <= 255 {
            self.cli.size.try_into().unwrap()
        } else {
            255
        };

        encoder
            .array(2)
            .unwrap()
            .u32(self.cli.addr)
            .unwrap()
            .u8(num_bytes)
            .unwrap()
            .end()
            .unwrap();

        self.cli.size -= u32::from(num_bytes);
        self.cli.addr += u32::from(num_bytes);

        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Post);
        request.set_path("/Memory");
        request
            .message
            .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
        request.message.set_payload(&buffer).unwrap();
        request
    }
}

impl CommandRegistry for MemRead {
    fn cmd() -> Command {
        Command {
            cmd: "MemRead".to_owned(),
            description: "Read arbitrary memory".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/Memory".to_owned()],
        }
    }

    fn parse(_cmd: &Command, args: String) -> Result<Box<dyn CommandHandler>, String> {
        let cli = MemReadCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
        Ok(Box::new(Self {
            buffer: Vec::new(),
            finished: false,
            displayable: false,
            cli,
        }))
    }
}

impl CommandHandler for MemRead {
    fn init(&mut self) -> CoapRequest<String> {
        self.send_request()
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        let mut decoder = Decoder::new(payload);
        if let Ok(bytes) = decoder.bytes() {
            self.buffer.extend_from_slice(bytes);
        }

        if self.cli.size > 0 {
            Some(self.send_request())
        } else {
            self.finished = true;
            self.displayable = true;
            None
        }
    }

    fn want_display(&self) -> bool {
        self.displayable
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn display(&self, buffer: &mut String) {
        let _ = writeln!(buffer, "Read {} byte(s).", self.buffer.len());
    }

    fn export(&self) -> Vec<u8> {
        self.buffer.clone()
    }
}
