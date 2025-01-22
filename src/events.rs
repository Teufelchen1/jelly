use std::fs::File;
use std::io::Stdout;
use std::io::Write;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::thread::JoinHandle;

use std::time::Duration;

use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use crate::slipmux::{SendPort, Transmit};
use serialport::SerialPort;

use crate::app::App;

pub enum Event {
    Diagnostic(String),
    Configuration(Vec<u8>),
    Packet(Vec<u8>),
    SerialConnect(Box<SendPort>),
    SerialDisconnect,
    TerminalKey(KeyEvent),
    TerminalMouse(MouseEvent),
    TerminalResize((), ()),
}

fn terminal_thread(sender: Sender<Event>) {
    loop {
        match crossterm::event::read().unwrap() {
            crossterm::event::Event::Key(key) => {
                let _ = sender.send(Event::TerminalKey(key));
            }
            crossterm::event::Event::Mouse(mouse) => {
                let _ = sender.send(Event::TerminalMouse(mouse));
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
        let event = match event_channel.recv_timeout(Duration::from_millis(100)) {
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
            Event::TerminalKey(key) => {
                if !app.on_key(key) {
                    break;
                }
            }
            Event::TerminalMouse(mouse) => {
                if !app.on_mouse(mouse) {
                    break;
                }
            }
            Event::TerminalResize(_, _) => (),
        };
    }
    let mut file = File::create("foo.txt").unwrap();
    for line in app.diagnostic_messages {
        file.write_all(line.to_string().as_bytes());
        file.write_all(b"\n");
    }
}
