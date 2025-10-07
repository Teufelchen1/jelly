use std::cmp::max;
use std::fmt::Write;
use std::time::SystemTime;

use chrono::prelude::DateTime;
use chrono::prelude::Utc;
use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::CoapResponse;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::Packet;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn token_to_u64(token: &[u8]) -> u64 {
    if token.len() > 8 {
        u64::MAX
    } else {
        let mut token_u64: u64 = 0;
        for byte in token {
            token_u64 += u64::from(*byte);
        }
        token_u64
    }
}

pub struct CoapLog {
    requests: Vec<Request>,
}

impl CoapLog {
    pub const fn new() -> Self {
        Self { requests: vec![] }
    }

    pub fn push(&mut self, request: CoapRequest<String>) {
        self.requests.push(Request::new(request));
    }

    pub fn to_paragraphs(&'_ self, short: bool) -> (u16, Vec<Paragraph<'_>>, Vec<Constraint>) {
        let mut req_blocks = vec![];
        let mut constrains = vec![];
        let total_length: u16 = {
            let mut sum = 0;
            // temporay limitation to work around ratatui bug #1855
            let start =
                usize::try_from(max(i64::try_from(self.requests.len()).unwrap() - 10, 0)).unwrap();
            for req in &self.requests[start..] {
                let (size, para) = if short {
                    req.paragraph_short()
                } else {
                    req.paragraph()
                };
                req_blocks.push(para);
                sum += size;
                constrains.push(Constraint::Length(size.try_into().unwrap()));
            }
            sum.try_into().unwrap_or(u16::MAX)
        };
        (total_length, req_blocks, constrains)
    }

    pub fn get_request_by_token(&mut self, token: u64) -> Option<&mut Request> {
        self.requests.iter_mut().find(|req| req.token == token)
    }
}

pub struct Request {
    pub time: SystemTime,
    pub req: CoapRequest<String>,
    pub token: u64,
    pub res: Vec<Response>,
}

impl Request {
    pub fn new(req: CoapRequest<String>) -> Self {
        let token = token_to_u64(req.message.get_token());
        Self {
            time: SystemTime::now(),
            req,
            token,
            res: vec![],
        }
    }

    pub fn add_response(&mut self, response: Packet) {
        self.res
            .push(Response::new(CoapResponse { message: response }));
    }

    fn get_request_title(&self) -> String {
        let mut out = String::new();
        let dt: DateTime<Utc> = self.time.into();
        _ = write!(out, "[{}]", dt.format("%H:%M:%S%.3f"));
        match self.req.message.header.code {
            MessageClass::Empty => _ = write!(out, "Empty"),
            MessageClass::Request(rtype) => {
                _ = write!(out, " ← Req({rtype:?} ");
                if let Some(option_list) = self.req.message.get_option(CoapOption::UriPath) {
                    for option in option_list {
                        _ = write!(out, "/{}", String::from_utf8_lossy(option));
                    }
                } else {
                    _ = write!(out, "/");
                }
                _ = write!(
                    out,
                    ")[0x{:04x}]",
                    u16::from_le_bytes(
                        self.req
                            .message
                            .get_token()
                            .try_into()
                            .unwrap_or([0xff, 0xff])
                    )
                );
            }
            MessageClass::Response(_) => _ = write!(out, "Response"),
            MessageClass::Reserved(_) => _ = write!(out, "Reserved"),
        }
        out
    }

    fn get_request_title_short(&self) -> String {
        let mut out = String::new();
        match self.req.message.header.code {
            MessageClass::Empty => _ = write!(out, "Empty"),
            MessageClass::Request(rtype) => {
                _ = write!(out, " ← Req({rtype:?} ");
                if let Some(option_list) = self.req.message.get_option(CoapOption::UriPath) {
                    for option in option_list {
                        _ = write!(out, "/{}", String::from_utf8_lossy(option));
                    }
                } else {
                    _ = write!(out, "/");
                }
                _ = write!(out, ")");
            }
            MessageClass::Response(_) => _ = write!(out, "Response"),
            MessageClass::Reserved(_) => _ = write!(out, "Reserved"),
        }
        out
    }

    pub fn paragraph(&self) -> (usize, Paragraph<'_>) {
        let block = Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .style(Style::new().gray())
            .title(vec![Span::from(self.get_request_title())])
            .title_alignment(Alignment::Left);

        if self.res.is_empty() {
            (3, Paragraph::new("Awaiting response").block(block))
        } else {
            let mut text = Text::default().reset_style();
            let mut header = Line::default();
            let timestamp = self.res[0].get_timestamp().gray();
            let status = self.res[0].get_status();
            let payload = Text::from(self.res[0].get_payload());

            header.push_span(timestamp);
            header.push_span(status);
            header.push_span(Span::default().reset_style());

            text.push_line(header);
            text.extend(payload);
            let size = text.lines.len() + 2;
            (size, Paragraph::new(text).block(block))
        }
    }

    pub fn paragraph_short(&self) -> (usize, Paragraph<'_>) {
        let block = Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .style(Style::new().gray())
            .title(self.get_request_title_short())
            .title_alignment(Alignment::Left);

        if self.res.is_empty() {
            (3, Paragraph::new("Awaiting response").block(block))
        } else {
            let mut text = Text::default().reset_style();
            let mut header = Line::default();
            let status = self.res[0].get_status_short();
            let payload = Text::from(self.res[0].get_payload());

            header.push_span(status);
            header.push_span(Span::default().reset_style());

            text.push_line(header);
            text.extend(payload);
            let size = text.lines.len() + 2;
            (size, Paragraph::new(text).block(block))
        }
    }
}

pub struct Response {
    time: SystemTime,
    coap: CoapResponse,
}

impl Response {
    pub fn new(coap: CoapResponse) -> Self {
        Self {
            time: SystemTime::now(),
            coap,
        }
    }

    fn get_timestamp(&self) -> String {
        let dt: DateTime<Utc> = self.time.into();
        format!("[{}]", dt.format("%H:%M:%S%.3f"))
    }

    fn get_status(&self) -> Span<'_> {
        let status = self.coap.get_status();
        let token = token_to_u64(self.coap.message.get_token());
        let bytes = self.coap.message.payload.len();

        let response_summary = self.coap.message.get_content_format().map_or_else(
            || format!(" → Res({status:?})[0x{token:03X}]: {bytes} bytes"),
            |c| format!(" → Res({status:?}/{c:?})[0x{token:03X}]: {bytes} bytes"),
        );

        if status.is_error() {
            Span::styled(response_summary, Style::new().red())
        } else {
            Span::styled(response_summary, Style::new().green())
        }
    }

    fn get_status_short(&self) -> Span<'_> {
        let status = self.coap.get_status();

        let response_summary = self.coap.message.get_content_format().map_or_else(
            || format!(" → Res({status:?})"),
            |c| format!(" → Res({status:?}/{c:?})"),
        );

        if status.is_error() {
            Span::styled(response_summary, Style::new().red())
        } else {
            Span::styled(response_summary, Style::new().green())
        }
    }

    pub fn get_payload(&self) -> String {
        let is_error = match self.coap.message.header.code {
            MessageClass::Response(response_type) => response_type.is_error(),
            // There is *definitely* an error if this happens, but for our purposes, we can't apply
            // the handling of diagnostic messages.
            _ => false,
        };

        let payload = &self.coap.message.payload;
        let payload_formatted = if payload.is_empty() {
            "Empty payload".to_owned()
        } else {
            match self.coap.message.get_content_format() {
                Some(ContentFormat::ApplicationLinkFormat) => {
                    String::from_utf8_lossy(payload).replace(",<", ",\n<")
                }
                Some(ContentFormat::TextPlain) => String::from_utf8_lossy(payload).to_string(),
                // Payload in errors w/o content format is called diagnostic messages and expected
                // to be text (see RFC7252 Section 5.5.2).
                None if is_error => String::from_utf8_lossy(payload).to_string(),
                // this is a cheap-in-terms-of-dependencies hex formatting; `aa bb cc` would be
                // prettier than `[aa, bb, cc]`, but needs extra dependencies.
                Some(ContentFormat::ApplicationCBOR) => {
                    cbor_edn::StandaloneItem::from_cbor(payload).map_or_else(
                        |e| format!("Parsing error {e}, content {payload:02x?}"),
                        |c| c.serialize(),
                    )
                }
                _ => {
                    format!("{payload:02x?}")
                }
            }
        };

        payload_formatted
    }
}
