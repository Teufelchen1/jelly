use std::io::Stdout;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crossterm::event::MouseEventKind;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::Cli;
use crate::Event;
use crate::EventChannel;
use crate::app::App;
use crate::create_network_thread;
use crate::create_slipmux_thread;

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

pub fn start_tui(args: Cli, main_channel: EventChannel) {
    fn reset_terminal() {
        crossterm::terminal::disable_raw_mode().unwrap();
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
            crossterm::cursor::Show
        )
        .unwrap();
    }

    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);

    let network_event_sender = if let Some(network_name) = args.network {
        let name = network_name.unwrap_or_else(|| "slip".to_owned());
        Some(create_network_thread(event_sender.clone(), &name))
    } else {
        None
    };

    // The UiState queries the terminal on creation for its colour theme
    // So we create it here early, before messing with the terminal ourselves
    let ui_state = UiState::new();

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        reset_terminal();
        original_hook(panic);
    }));

    crossterm::terminal::enable_raw_mode().unwrap();
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        crossterm::cursor::Hide
    )
    .unwrap();
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout)).unwrap();

    terminal.clear().unwrap();

    create_terminal_thread(event_sender.clone());

    let app = App::new(event_sender);

    event_loop_tui(
        app,
        ui_state,
        &event_receiver,
        &slipmux_event_sender,
        network_event_sender.as_ref(),
        terminal,
    );

    reset_terminal();
}

pub fn event_loop_tui(
    mut app: App,
    mut ui_state: UiState,
    event_channel: &Receiver<Event>,
    hardware_event_sender: &Sender<Event>,
    network_event_sender: Option<&Sender<Event>>,
    mut terminal: Terminal<CrosstermBackend<Stdout>>,
) {
    terminal
        .draw(|frame| app.draw(&mut ui_state, frame))
        .unwrap();

    loop {
        let event = match event_channel.recv_timeout(Duration::from_millis(1000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => {
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };

        match event {
            Event::Diagnostic(msg) => {
                app.on_diagnostic_msg(&msg);
                ui_state.get_dirty_from_tab(SelectedTab::Diagnostic);
                ui_state.get_dirty_from_tab(SelectedTab::Overview);
            }
            Event::Configuration(data) => {
                app.on_configuration_msg(Some(&mut ui_state), &data);
                ui_state.get_dirty_from_tab(SelectedTab::Configuration);
                ui_state.get_dirty_from_tab(SelectedTab::Overview);
            }
            Event::Packet(packet) => {
                app.on_packet(&packet);
                if let Some(n_e_sender) = network_event_sender {
                    n_e_sender.send(Event::Packet(packet)).unwrap();
                }
                ui_state.get_dirty_from_tab(SelectedTab::Net);
            }
            Event::SendDiagnostic(d) => hardware_event_sender
                .send(Event::SendDiagnostic(d.to_string()))
                .unwrap(),
            Event::SendConfiguration(c) => {
                hardware_event_sender
                    .send(Event::SendConfiguration(c))
                    .unwrap();

                ui_state.get_dirty_from_tab(SelectedTab::Configuration);
                ui_state.get_dirty_from_tab(SelectedTab::Overview);
            }
            Event::SendPacket(packet) => {
                app.off_packet(&packet);
                hardware_event_sender
                    .send(Event::SendPacket(packet))
                    .unwrap();
                ui_state.get_dirty_from_tab(SelectedTab::Net);
            }
            Event::SerialConnect(tty_name) => {
                app.on_connect();
                ui_state.set_device_path(tty_name);
            }
            Event::SerialDisconnect => {
                app.on_disconnect();
                ui_state.clear_device_path();
            }
            Event::TerminalString(_msg) => (),
            Event::TerminalKey(key) => {
                if !app.on_key(Some(&mut ui_state), key) {
                    break;
                }
            }
            Event::TerminalMouse(mouse) => {
                ui_state.on_mouse(mouse);
            }
            Event::TerminalResize | Event::TerminalEOF => (),
            Event::NetworkConnect(interface_name) => {
                ui_state.set_iface_name(interface_name);
            }
        }

        if ui_state.is_dirty() {
            terminal
                .draw(|frame| app.draw(&mut ui_state, frame))
                .unwrap();
            ui_state.wash();
        }
    }
}
