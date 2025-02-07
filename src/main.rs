//#![feature(trait_upcasting)]
use std::fs;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use clap::Parser;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::events::create_terminal_thread;
use crate::events::event_loop;
use crate::events::Event;
use crate::slipmux::create_slipmux_thread;

mod app;
mod events;
mod slipmux;

#[derive(Parser)]
struct Cli {
    /// The path to the UART TTY interface
    tty_path: std::path::PathBuf,
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
    let metadata = fs::metadata(args.tty_path.clone()).unwrap();
    let filetype = metadata.file_type();
    // if filetype.is_char_device() {
    //     println!("{} is not a character device.", args.tty_path.display());
    //     return;
    // }
    let path = args.tty_path.to_str().unwrap();

    let (event_sender, event_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();
    let slipmux_event_sender = event_sender.clone();
    let terminal_event_sender = event_sender.clone();
    let _ = create_slipmux_thread(slipmux_event_sender, path.to_string());
    let _ = create_terminal_thread(terminal_event_sender);

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

    event_loop(event_receiver, terminal);

    reset_terminal();
}
