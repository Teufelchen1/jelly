use std::io::Stdout;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crossterm::event::MouseEventKind;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::Event;

pub use ui_state::SelectedTab;
pub use ui_state::UiState;

mod render;
mod ui_state;

fn terminal_thread(sender: &Sender<Event>) {
    loop {
        match crossterm::event::read().unwrap() {
            crossterm::event::Event::Key(key) => {
                sender.send(Event::TerminalKey(key)).unwrap();
            }
            crossterm::event::Event::Mouse(mouse) => {
                if matches!(
                    mouse.kind,
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
                ) {
                    sender.send(Event::TerminalMouse(mouse)).unwrap();
                }
            }
            crossterm::event::Event::Resize(_columns, _rows) => {
                sender.send(Event::TerminalResize).unwrap();
            }
            _ => (),
        }
    }
}

fn create_terminal_thread(sender: Sender<Event>) -> JoinHandle<()> {
    thread::spawn(move || terminal_thread(&sender))
}

pub fn tui_event_loop(
    event_channel: &Receiver<Event>,
    event_sender: Sender<Event>,
    hardware_event_sender: &Sender<Event>,
    mut terminal: Terminal<CrosstermBackend<Stdout>>,
) {
    create_terminal_thread(event_sender.clone());

    let mut app = App::new(event_sender);
    terminal.draw(|frame| app.draw(frame)).unwrap();

    loop {
        let event = match event_channel.recv_timeout(Duration::from_millis(1000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => {
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        match event {
            Event::Diagnostic(msg) => app.on_diagnostic_msg(&msg),
            Event::Configuration(data) => app.on_configuration_msg(&data),
            Event::Packet(_data) => (),
            Event::SendDiagnostic(d) => hardware_event_sender
                .send(Event::SendDiagnostic(d))
                .unwrap(),
            Event::SendConfiguration(c) => hardware_event_sender
                .send(Event::SendConfiguration(c))
                .unwrap(),
            Event::SerialConnect(name) => app.on_connect(name),
            Event::SerialDisconnect => app.on_disconnect(),
            Event::TerminalString(_msg) => (),
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
            Event::TerminalResize => (),
        }
        terminal.draw(|frame| app.draw(frame)).unwrap();
    }
}
