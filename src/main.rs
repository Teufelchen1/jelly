//#![feature(trait_upcasting)]
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::time::Duration;

use clap::Parser;
use coap_lite::CoapOption;
use coap_lite::Packet;
use commands::CommandLibrary;
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
        let lib = CommandLibrary::default();
        if let Some(cmd) = lib.find_by_cmd(&cmd_str.split(' ').next().unwrap()) {
            let handler = cmd.handler.unwrap();
            let display = cmd.display.unwrap();
            if let Ok(coap) = handler(cmd_str, &cmd.location.as_ref().unwrap()) {
                let mut msg = coap.message;
                msg.header.message_id = rand::rng().random();
                msg.set_token(1312u16.to_le_bytes().to_vec());
                msg.add_option(CoapOption::Block2, vec![0x05]);

                let (data, size) = encode_configuration(msg.to_bytes().unwrap());
                while let Ok(event) = event_receiver.recv_timeout(Duration::from_secs(5)) {
                    match event {
                        Event::Diagnostic(msg) => {println!("{:}", msg)},
                        Event::Configuration(data) => {
                            println!("Got conf: {:?}", data);
                            let response = Packet::from_bytes(&data).unwrap();
                            println!("{:}", display(response.payload));
                        },
                        Event::SerialConnect(name) => {
                            println!("Serial connect :) {:}", name);
                            let _ = hardware_event_sender.send(Event::SendConfiguration(data[..size].to_vec()));
                        },
                        Event::SerialDisconnect =>{println!("Serial disconnect :(")},
                        _ => {},
                    }
                }
            }
        }
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
