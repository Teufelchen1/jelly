use std::fmt::Write;
use std::fs::File;
use std::io::Write as FileWrite;
use std::time::SystemTime;

use chrono::prelude::DateTime;
use chrono::prelude::Utc;
use coap_lite::CoapRequest;
use coap_lite::CoapResponse;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::Packet;
use ratatui::prelude::Alignment;
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::app::CoapOption;
use crate::command::CommandHandler;

pub enum SaveToFile {
    No,
    AsBin(String),
    AsText(String),
    ToStdout,
}

pub struct Job {
    handler: Option<Box<dyn CommandHandler>>,
    file: SaveToFile,
    cli: String,
    log: String,
    start_time: SystemTime,
    end_time: Option<SystemTime>,
    finished: bool,
}

fn hexdump(bin_data: &[u8]) -> String {
    let mut buffer = String::new();
    writeln!(buffer, "\n   |0 1 2 3  4 5 6 7  8 9 A B  C D E F").unwrap();
    for (index, chunk) in bin_data.chunks(16).enumerate() {
        write!(buffer, "{:03X}|", index * 16).unwrap();
        for minichunk in chunk.chunks(4) {
            for byte in minichunk {
                write!(buffer, "{byte:02X}").unwrap();
            }
            write!(buffer, " ").unwrap();
        }
        writeln!(buffer).unwrap();
    }
    buffer
}

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

impl Job {
    pub fn new(handler: Box<dyn CommandHandler>, file: SaveToFile, cli: String) -> Self {
        Self {
            handler: Some(handler),
            file,
            cli,
            log: String::new(),
            start_time: SystemTime::now(),
            end_time: None,
            finished: false,
        }
    }

    pub fn new_failed(cli: String, err: &str) -> Self {
        Self {
            handler: None,
            file: SaveToFile::No,
            cli,
            log: err.to_owned(),
            start_time: SystemTime::now(),
            end_time: None,
            finished: false,
        }
    }

    fn finish(&mut self) {
        self.end_time = Some(SystemTime::now());
        self.finished = true;
    }

    fn handle_display(&mut self) -> std::string::String {
        let mut buffer = String::new();
        match self.file {
            SaveToFile::No => {
                self.handler.as_mut().unwrap().display(&mut buffer);
            }
            SaveToFile::AsBin(ref file) => {
                let bin_data: Vec<u8> = self.handler.as_mut().unwrap().export();
                buffer.push_str(&hexdump(&bin_data));
                match File::create(file) {
                    Ok(mut f) => {
                        f.write_all(&bin_data).unwrap();
                        let _ = write!(buffer, "{}", &format!("(binary saved to: {file})\n"));
                    }
                    Err(e) => {
                        let _ = write!(buffer, "{}", &format!("(unable to write to {file}: {e}"));
                    }
                }
            }
            SaveToFile::AsText(ref file) => {
                self.handler.as_mut().unwrap().display(&mut buffer);
                match File::create(file) {
                    Ok(mut f) => {
                        f.write_all(buffer.as_bytes()).unwrap();
                        let _ = write!(buffer, "{}", &format!("(saved to: {file})\n"));
                    }
                    Err(e) => {
                        let _ = write!(buffer, "{}", &format!("(unable to write to {file}: {e}"));
                    }
                }
            }
            SaveToFile::ToStdout => {
                let bin_data: Vec<u8> = self.handler.as_mut().unwrap().export();
                std::io::stdout().write_all(&bin_data).unwrap();
            }
        }
        self.log.push_str(&buffer);
        buffer
    }

    pub fn paragraph(&self) -> (usize, Paragraph<'_>) {
        let dt: DateTime<Utc> = self.start_time.into();
        let block = Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .style(Style::new().gray())
            .title(format!("[{}] {}", dt.format("%H:%M:%S%.3f"), self.cli))
            .title_alignment(Alignment::Left);

        let size = self.log.lines().count() + 2;
        (
            size,
            Paragraph::new(self.log.clone())
                .wrap(Wrap { trim: false })
                .block(block)
                .set_style(Style::reset()),
        )
    }
}

pub struct JobLog {
    pub jobs: Vec<Job>,
}

impl JobLog {
    pub const fn new() -> Self {
        Self { jobs: vec![] }
    }

    pub fn job_handle_response(
        &mut self,
        job_id: usize,
        response: &Packet,
    ) -> Option<CoapRequest<String>> {
        self.jobs[job_id].handler.as_mut().unwrap().handle(response)
    }

    pub fn job_wants_display(&self, job_id: usize) -> bool {
        if self.jobs[job_id].handler.is_some() {
            self.jobs[job_id].handler.as_ref().unwrap().want_display()
        } else {
            false
        }
    }

    pub fn job_display(&mut self, job_id: usize) -> String {
        if self.jobs[job_id].handler.is_some() {
            self.jobs[job_id].handle_display()
        } else {
            self.jobs[job_id].log.clone()
        }
    }

    pub fn job_is_finished(&self, job_id: usize) -> bool {
        if self.jobs[job_id].handler.is_some() {
            self.jobs[job_id].handler.as_ref().unwrap().is_finished()
        } else {
            true
        }
    }

    pub fn job_finish(&mut self, job_id: usize) {
        self.jobs[job_id].finish();
    }

    pub fn start(&mut self, job: Job) -> usize {
        self.jobs.push(job);
        self.jobs.len() - 1
    }

    pub fn dump(&self) -> Vec<String> {
        let mut dump = vec![];
        for job in &self.jobs {
            dump.push(job.log.clone());
        }
        dump
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

pub struct Request {
    pub time: SystemTime,
    pub req: CoapRequest<String>,
    pub token: u64,
    pub res: Vec<Response>,
}

impl Request {
    pub fn new(req: CoapRequest<String>) -> Self {
        let mut token: u64 = 0;
        for byte in req.message.get_token() {
            token += u64::from(*byte);
        }
        Self {
            time: SystemTime::now(),
            req,
            token,
            res: vec![],
        }
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
