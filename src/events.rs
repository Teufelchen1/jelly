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
    Diagnostic(String),
    Configuration(Vec<u8>),
    Packet(Vec<u8>),
    SerialConnect(Box<dyn SerialPort>),
    SerialDisconnect,
    Terminal(KeyEvent),
    TerminalResize(u16, u16),
}

fn terminal_thread(sender: Sender<Event>) {
    loop {
        match crossterm::event::read().unwrap() {
            crossterm::event::Event::Key(key) => {
                let _ = sender.send(Event::Terminal(key));
            }
            crossterm::event::Event::Resize(columns, rows) => {
                let _ = sender.send(Event::TerminalResize(columns, rows));
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
) {
    let mut app = App::new();
    loop {
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let event = match event_channel.recv_timeout(Duration::from_millis(20000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        let okay = match event {
            Event::Diagnostic(msg) => {
                app.on_diagnostic_msg(msg);
                true
            }
            Event::Configuration(data) => {
                app.on_configuration_msg(data);
                true
            }
            Event::Packet(data) => true,
            Event::SerialConnect(write_port) => {
                app.connect(write_port);
                true
            }
            Event::SerialDisconnect => {
                app.disconnect();
                true
            }
            Event::Terminal(key) => app.on_key(key),
            Event::TerminalResize(_, _) => true,
        };
        if !okay {
            break;
        }
    }
}
