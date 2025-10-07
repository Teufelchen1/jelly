use std::time::SystemTime;

use chrono::prelude::DateTime;
use chrono::prelude::Utc;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

pub struct Diagnostic {
    pub time: SystemTime,
    pub message: String,
}

pub struct DiagnosticLog {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticLog {
    pub const fn new() -> Self {
        Self {
            diagnostics: vec![],
        }
    }

    fn add_new_empty(&mut self, time: SystemTime) {
        let new = Diagnostic {
            time,
            message: String::new(),
        };

        self.diagnostics.push(new);
    }

    fn last_or_new(&mut self, time: SystemTime) -> &mut Diagnostic {
        if self.diagnostics.is_empty() {
            self.add_new_empty(time);
        }
        self.diagnostics.last_mut().unwrap()
    }

    // This has to do a bit of lifting
    // We want each output line to be a single String in a Diagnostic object.
    // But the client might send as multiple messages which are all part of the same output line.
    // e.g. ["foo", "bar", "baz\n", "hello wo", "rld!\n"] shoud be transformed into two objects:
    // -> [Diagnostic("foobarbaz"), Diagnostic("hello world!")]
    // In addition, we need to filterout control chars.
    pub fn add(&mut self, message: &str) {
        let time = SystemTime::now();

        if message == "\n" {
            self.add_new_empty(time);
            return;
        }
        // Split new lines, as each line should get it's own Diagnostic object
        let messages = message.split('\n');
        let mut iter = messages.peekable();
        while let Some(message) = iter.next() {
            // Split returns empty strings if a new line is isolated
            if message.is_empty() {
                self.add_new_empty(time);
                continue;
            }
            // Remove control chars which would mess with ratatui
            let mut message = message.to_owned();
            message.retain(|c| c != '\r' && c != '\t');

            // Add string to last Diagnostic
            let last = self.last_or_new(time);
            last.message.push_str(&message);

            // If there is a next line, consider this one finished
            // Unless the next line is empty, in that case, the next iteration will do it.
            if iter.peek().is_some_and(|msg| !msg.is_empty()) {
                self.add_new_empty(time);
            }
        }
    }

    pub fn paragraph(&self) -> (usize, Paragraph<'_>) {
        let mut lines = vec![];
        for diag in &self.diagnostics {
            let dt: DateTime<Utc> = diag.time.into();
            lines.push(Line::from(format!(
                "[{}] {}",
                dt.format("%H:%M:%S%.3f"),
                diag.message.as_str()
            )));
        }
        (
            lines.len(),
            Paragraph::new(lines).wrap(Wrap { trim: false }),
        )
    }

    pub fn paragraph_short(&self) -> (usize, Paragraph<'_>) {
        let mut lines = vec![];
        for diag in &self.diagnostics {
            lines.push(Line::from(diag.message.as_str()));
        }
        (
            lines.len(),
            Paragraph::new(lines).wrap(Wrap { trim: false }),
        )
    }
}
