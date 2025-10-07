use std::fmt::Write;
use std::fs::File;
use std::io::Write as FileWrite;
use std::time::SystemTime;

use chrono::prelude::DateTime;
use chrono::prelude::Utc;
use coap_lite::CoapRequest;
use coap_lite::Packet;
use ratatui::prelude::Alignment;
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::command::CommandHandler;

pub type JobId = usize;

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
        job_id: JobId,
        response: &Packet,
    ) -> Option<CoapRequest<String>> {
        self.jobs[job_id].handler.as_mut().unwrap().handle(response)
    }

    pub fn job_wants_display(&self, job_id: JobId) -> bool {
        if self.jobs[job_id].handler.is_some() {
            self.jobs[job_id].handler.as_ref().unwrap().want_display()
        } else {
            false
        }
    }

    pub fn job_display(&mut self, job_id: JobId) -> String {
        if self.jobs[job_id].handler.is_some() {
            self.jobs[job_id].handle_display()
        } else {
            self.jobs[job_id].log.clone()
        }
    }

    pub fn job_is_finished(&self, job_id: JobId) -> bool {
        if self.jobs[job_id].handler.is_some() {
            self.jobs[job_id].handler.as_ref().unwrap().is_finished()
        } else {
            true
        }
    }

    pub fn job_finish(&mut self, job_id: JobId) {
        self.jobs[job_id].finish();
    }

    pub fn start(&mut self, job: Job) -> JobId {
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
