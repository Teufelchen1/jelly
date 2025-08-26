use crate::Event;
use std::io::stdin;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

pub use configuration::event_loop_configuration;
pub use diagnostic::event_loop_diagnostic;

mod configuration;
mod diagnostic;

fn raw_terminal_thread(sender: &Sender<Event>) {
    let mut buffer = String::new();
    loop {
        if let Ok(len) = stdin().read_line(&mut buffer) {
            if len == 0 {
                sender.send(Event::TerminalEOF).unwrap();
                return;
            }
            sender.send(Event::TerminalString(buffer.clone())).unwrap();
        }
        buffer.clear();
    }
}

fn create_raw_terminal_thread(sender: Sender<Event>) -> JoinHandle<()> {
    thread::spawn(move || raw_terminal_thread(&sender))
}
