use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use super::log::Log;
use super::log::LogEntry;

impl Log<String> {
    fn add_new_empty(&mut self) {
        self.add_new(String::default());
    }

    fn last_or_new(&mut self) -> &mut LogEntry<String> {
        if self.is_empty() {
            self.add_new_empty();
        }
        self.last_mut()
            .expect("We ensured the log is not empty, can not fail.")
    }

    // This has to do a bit of lifting
    // We want each log line to be a single String in a LogEntry object.
    // But the client might send us multiple messages which are all part of the same output line.
    // This means repeatedly calling this function, each time with the next part of the message.
    // e.g. ["foo", "bar", "baz\n", "hello wo", "rld!\n"] shoud be transformed into two objects:
    // -> [LogEntry("foobarbaz"), LogEntry("hello world!")]
    // In addition, we need to filterout control chars (for ratatui / displaying).
    pub fn append_message(&mut self, message: &str) {
        if message == "\n" {
            self.add_new_empty();
            return;
        }
        // Split new lines, as each line should get it's own Diagnostic object
        let messages = message.split('\n');
        let mut iter = messages.peekable();
        while let Some(message) = iter.next() {
            // Split returns empty strings if a new line is isolated
            if message.is_empty() {
                self.add_new_empty();
                continue;
            }
            // Remove control chars which would mess with ratatui
            let mut message = message.to_owned();
            message.retain(|c| c != '\r' && c != '\t');

            // Add string to last Diagnostic
            let last = self.last_or_new();
            last.data.push_str(&message);

            // If there is a next line, consider this one finished
            // Unless the next line is empty, in that case, the next iteration will do it.
            if iter.peek().is_some_and(|msg| !msg.is_empty()) {
                self.add_new_empty();
            }
        }
    }

    pub fn paragraph(&self) -> (usize, Paragraph<'_>) {
        let mut lines = vec![];
        for entry in self {
            let timestamp = entry.get_timestamp();
            lines.push(Line::from(format!("{timestamp} {}", entry.data.as_str())));
        }
        (
            lines.len(),
            Paragraph::new(lines).wrap(Wrap { trim: false }),
        )
    }

    pub fn paragraph_short(&self) -> (usize, Paragraph<'_>) {
        let mut lines = vec![];
        for entry in self {
            lines.push(Line::from(entry.data.as_str()));
        }
        (
            lines.len(),
            Paragraph::new(lines).wrap(Wrap { trim: false }),
        )
    }
}
