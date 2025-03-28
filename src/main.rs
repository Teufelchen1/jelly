//#![feature(trait_upcasting)]
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use clap::Parser;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::events::create_terminal_thread;
use crate::events::event_loop;
use crate::events::Event;
use crate::hardware::create_slipmux_thread;

mod app;
mod events;
mod hardware;
mod slipmux;
mod transport;

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
    // if args
    //     .tty_path
    //     .metadata()
    //     .expect("Could not read metadata of tty-path")
    //     .file_type()
    //     .is_char_device()
    // {
    //     println!("{} is not a character device.", args.tty_path.display());
    //     return;
    // }

    let (event_sender, event_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();
    create_slipmux_thread(event_sender.clone(), args.tty_path);
    create_terminal_thread(event_sender);

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

    event_loop(&event_receiver, terminal);

    reset_terminal();
}
