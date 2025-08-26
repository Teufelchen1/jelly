use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use clap::Parser;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::events::Event;
use crate::headless::event_loop_configuration;
use crate::headless::event_loop_diagnostic;
use crate::slipmux::create_slipmux_thread;
use crate::tui::event_loop_tui;

mod app;
mod command;
mod events;
mod headless;
mod slipmux;
mod transport;
mod tui;

type EventChannel = (Sender<Event>, Receiver<Event>);

#[derive(Parser)]
struct Cli {
    /// The path to the UART TTY interface
    tty_path: std::path::PathBuf,

    /// If true, disables the TUI and passes diagnostic messages via stdio
    ///
    /// This is interactive. Jelly will await input and output indefinitely.
    /// Configuration messages are ignored.
    /// This means that any pre-known or configuration-based commands are not
    /// available.
    #[arg(short = 'd', long, default_value_t = false, verbatim_doc_comment)]
    headless_diagnostic: bool,

    /// If true, disables the TUI and passes configuration messages via stdio
    ///
    /// Use this mode inside scripts and pipe commands into Jelly.
    /// This may be used interactive.
    /// Jelly will await input unitl EOF.
    /// Jelly will wait for output until all commands are finished or
    /// the time-out is reached. The output will only be displayed once EOF is
    /// reached. This is to preserve the order of input commands regardless of
    /// the commands run time.
    /// Diagnostic messages are ignored.
    /// Pre-known, configuration-based commands are available.
    #[arg(short = 'c', long, default_value_t = false, verbatim_doc_comment)]
    headless_configuration: bool,
}

fn start_headless_diagnostic(args: Cli, main_channel: EventChannel) {
    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);
    event_loop_diagnostic(&event_receiver, event_sender, &slipmux_event_sender);
}

fn start_headless_configuration(args: Cli, main_channel: EventChannel) {
    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);
    event_loop_configuration(&event_receiver, event_sender, &slipmux_event_sender);
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

    event_loop_tui(
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

    if args.headless_diagnostic {
        start_headless_diagnostic(args, main_channel);
    } else if args.headless_configuration {
        start_headless_configuration(args, main_channel);
    } else {
        start_tui(args, main_channel);
        println!("Thank you for using Jelly 🪼");
    }
}
