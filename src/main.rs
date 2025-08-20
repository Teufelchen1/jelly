use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use clap::Parser;
use events::event_loop_headless;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::events::event_loop;
use crate::events::Event;
use crate::slipmux::create_slipmux_thread;

mod app;
mod command;
mod events;
mod slipmux;
mod transport;
mod tui;

type EventChannel = (Sender<Event>, Receiver<Event>);

#[derive(Parser)]
struct Cli {
    /// The path to the UART TTY interface
    tty_path: std::path::PathBuf,

    /// If true, disables the TUI and passes diagnostic messages via stdio
    #[arg(long, default_value_t = false)]
    headless: bool,
}

fn start_headless(args: Cli, main_channel: EventChannel) {
    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);
    event_loop_headless(&event_receiver, event_sender, &slipmux_event_sender);
}

fn start_tui(args: Cli, main_channel: EventChannel) {
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
        &slipmux_event_sender,
        terminal,
    );

    reset_terminal();
}

fn main() {
    let args = Cli::parse();
    if !args.tty_path.exists() {
        println!("{} could not be found.", args.tty_path.display());
        return;
    }

    let main_channel: EventChannel = mpsc::channel();

    if args.headless {
        start_headless(args, main_channel);
    } else {
        start_tui(args, main_channel);
    }
    println!("Thank you for using Jelly 🪼");
}
