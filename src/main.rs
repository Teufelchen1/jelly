use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use clap::Parser;
use coap_lite::CoapOption;
use coap_lite::Packet;
use commands::CommandLibrary;
use events::event_one_shot;
use rand::Rng;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use slipmux::encode_configuration;

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
    /// Runs a single jelly command, awaits response and prints the result
    cmd: Option<String>,
}

fn one_shot_command(
    event_receiver: &Receiver<Event>,
    hardware_event_sender: &Sender<Event>,
    input_str: String,
) {
    let lib = CommandLibrary::default();
    let mut cmd_iter = input_str.split('|');
    let cmd_str = cmd_iter.next().unwrap();
    let handler_selection = cmd_iter.next().unwrap_or("as_text").trim();
    if let Some(cmd) = lib.find_by_cmd(cmd_str.split(' ').next().unwrap()) {
        let handler = cmd.handler.unwrap();
        if let Ok(coap) = handler(cmd_str.to_string(), cmd.location.as_ref().unwrap()) {
            let mut msg = coap.message;
            msg.header.message_id = rand::rng().random();
            msg.set_token(1312u16.to_le_bytes().to_vec());
            msg.add_option(CoapOption::Block2, vec![0x05]);

            let (data, size) = encode_configuration(msg.to_bytes().unwrap());

            match event_one_shot(event_receiver, hardware_event_sender, &data[..size]) {
                Ok(data) => {
                    match handler_selection {
                        "as_cbor" => {
                            let display = cmd.displayCbor.unwrap();
                            let _ = std::io::Write::write(&mut std::io::stdout(), &display(data));
                        }
                        "as_text" | _ => {
                            let display = cmd.display.unwrap();
                            println!("{:}", display(data));
                        },
                    };
                }
                Err(err) => {
                    println!("{err}");
                }
            }
        } else {
            println!("Unable to parse command arguments for: {:}, got {:}", cmd.cmd, cmd_str);
        }
    } else {
        println!("No such command: {cmd_str}");
    }
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

    let (hardware_event_sender, hardware_event_receiver): (Sender<Event>, Receiver<Event>) =
        mpsc::channel();
    let (event_sender, event_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();

    create_slipmux_thread(event_sender.clone(), hardware_event_receiver, args.tty_path);

    if let Some(cmd_str) = args.cmd {
        one_shot_command(&event_receiver, &hardware_event_sender, cmd_str);
        return;
    }

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
    println!("Thank you for using Jelly 🪼");
}
