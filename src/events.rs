use std::io::Stdout;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::thread::JoinHandle;

use std::time::Duration;

use crossterm::event::KeyEvent;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use serialport::SerialPort;

use crate::tui2::App;

pub enum Event {
    Fifo(String),
    Diagnostic(String),
    Configuration(Vec<u8>),
    Packet(Vec<u8>),
    Terminal(KeyEvent),
    TerminalResize(u16, u16),
}

fn terminal_thread(sender: Sender<Event>) {
    loop {
        match crossterm::event::read().unwrap() {
            crossterm::event::Event::Key(key) => {
                sender.send(Event::Terminal(key));
            }
            crossterm::event::Event::Resize(columns, rows) => {
                sender.send(Event::TerminalResize(columns, rows));
            }
            _ => (),
        };
    }
}

pub fn create_terminal_thread(sender: Sender<Event>) -> JoinHandle<()> {
    thread::spawn(move || terminal_thread(sender))
}

pub fn event_loop(
    event_channel: Receiver<Event>,
    mut terminal: Terminal<CrosstermBackend<Stdout>>,
    write_port: Box<dyn SerialPort>,
) {
    let mut app = App::new(write_port);
    loop {
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let event = match event_channel.recv_timeout(Duration::from_millis(20000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => todo!(),
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        let okay = match event {
            Event::Fifo(txt) => true,
            Event::Diagnostic(msg) => {
                app.on_diagnostic_msg(msg);
                true
            }
            Event::Configuration(data) => true,
            Event::Packet(data) => true,
            Event::Terminal(key) => app.on_key(key),
            Event::TerminalResize(_, _) => true,
        };
        if !okay {
            break;
        }
    }
}
