use std::io::Stdout;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::thread::JoinHandle;

use std::time::Duration;

use crossterm::event::KeyEvent;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use serialport::SerialPort;

use crate::app::App;

pub enum Event {
    Diagnostic(String),
    Configuration(Vec<u8>),
    Packet(Vec<u8>),
    SerialConnect(Box<dyn SerialPort>),
    SerialDisconnect,
    Terminal(KeyEvent),
    TerminalResize((), ()),
}

fn terminal_thread(sender: Sender<Event>) {
    loop {
        match crossterm::event::read().unwrap() {
            crossterm::event::Event::Key(key) => {
                let _ = sender.send(Event::Terminal(key));
            }
            crossterm::event::Event::Resize(_columns, _rows) => {
                let _ = sender.send(Event::TerminalResize((), ()));
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
        let event = match event_channel.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => {
                terminal.draw(|frame| app.draw(frame)).unwrap();
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        match event {
            Event::Diagnostic(msg) => {
                app.on_diagnostic_msg(msg);
            }
            Event::Configuration(data) => {
                app.on_configuration_msg(data);
            }
            Event::Packet(_data) => (),
            Event::SerialConnect(write_port) => {
                app.connect(write_port);
            }
            Event::SerialDisconnect => {
                app.disconnect();
            }
            Event::Terminal(key) => {
                if !app.on_key(key) {
                    break;
                }
            }
            Event::TerminalResize(_, _) => (),
        };
    }
}
