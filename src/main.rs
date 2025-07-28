use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use clap::Parser;
use events::event_loop_headless;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::events::create_terminal_thread;
use crate::events::event_loop;
use crate::events::Event;
use crate::hardware::create_slipmux_thread;

mod app;
mod commands;
mod events;
mod hardware;
mod transport;

#[derive(Parser)]
struct Cli {
    /// The path to the UART TTY interface
    tty_path: std::path::PathBuf,

    /// If true, disables the TUI and passes diagnostic messages via stdio
    #[arg(long, default_value_t = false)]
    headless: bool,
}

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

fn main() {
    let args = Cli::parse();
    if !args.tty_path.exists() {
        println!("{} could not be found.", args.tty_path.display());
        return;
    }

    if args.headless {
        let (hardware_event_sender, hardware_event_receiver): (Sender<Event>, Receiver<Event>) =
            mpsc::channel();
        let (event_sender, event_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();

        create_slipmux_thread(event_sender.clone(), hardware_event_receiver, args.tty_path);

        event_loop_headless(&event_receiver, event_sender, &hardware_event_sender);
    } else {
        let (hardware_event_sender, hardware_event_receiver): (Sender<Event>, Receiver<Event>) =
            mpsc::channel();
        let (event_sender, event_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();

        create_slipmux_thread(event_sender.clone(), hardware_event_receiver, args.tty_path);
        create_terminal_thread(event_sender.clone());

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

        event_loop(
            &event_receiver,
            event_sender,
            &hardware_event_sender,
            terminal,
        );

        reset_terminal();
    }
    println!("Thank you for using Jelly 🪼");
}
