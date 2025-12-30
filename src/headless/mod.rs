use std::io::stdin;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::Cli;
use crate::Event;
use crate::EventChannel;
use crate::create_network_thread;
use crate::create_slipmux_thread;
use crate::mpsc::RecvTimeoutError;

use configuration::event_loop_configuration;
use diagnostic::event_loop_diagnostic;
use diagnostic_network::event_loop_diagnostic_network;
use network::event_loop_network;

mod configuration;
mod diagnostic;
mod diagnostic_network;
mod network;

// The first event must be the serial connect. We wait for that event before processing
// stdin. If we were to process stdin first, we might try to send data to the device
// before it is even connected. This data would be lost.
fn await_serial_connect(event_channel: &Receiver<Event>) -> Result<String, &str> {
    let event = match event_channel.recv_timeout(Duration::from_millis(5000)) {
        Ok(event) => event,
        Err(RecvTimeoutError::Timeout) => {
            return Err("Timedout while waiting for serial to connect.");
        }
        Err(RecvTimeoutError::Disconnected) => panic!(),
    };
    if let Event::SerialConnect(name) = event {
        return Ok(name);
    }
    Err("Unknown event occoured while waiting for serial to connect.")
}

fn raw_terminal_thread(sender: &Sender<Event>) {
    let mut buffer = String::new();
    loop {
        if let Ok(len) = stdin().read_line(&mut buffer) {
            if len == 0 {
                sender.send(Event::TerminalEOF).unwrap();
                return;
            }
            sender.send(Event::TerminalString(buffer.clone())).unwrap();
        }
        buffer.clear();
    }
}

fn create_raw_terminal_thread(sender: Sender<Event>) -> JoinHandle<()> {
    thread::spawn(move || raw_terminal_thread(&sender))
}

pub fn start_headless_diagnostic(args: Cli, main_channel: EventChannel) {
    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);
    match await_serial_connect(&event_receiver) {
        Ok(name) => {
            println!("Serial connect with {name}");
            create_raw_terminal_thread(event_sender);
        }
        Err(e) => {
            println!("{e}");
            return;
        }
    }
    event_loop_diagnostic(&event_receiver, &slipmux_event_sender);
}

pub fn start_headless_configuration(args: Cli, main_channel: EventChannel) {
    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);
    event_loop_configuration(&event_receiver, event_sender, &slipmux_event_sender);
}

pub fn start_headless_network(args: Cli, main_channel: EventChannel) {
    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);

    match await_serial_connect(&event_receiver) {
        Ok(name) => {
            println!("Serial connect with {name}");
            create_raw_terminal_thread(event_sender.clone());
        }
        Err(e) => {
            println!("{e}");
            return;
        }
    }

    let name = args.network.flatten().unwrap_or_else(|| "slip".to_owned());
    let network_event_sender = create_network_thread(event_sender, &name);

    event_loop_network(
        &event_receiver,
        &slipmux_event_sender,
        &network_event_sender,
    );
}

pub fn start_headless_diagnostic_network(args: Cli, main_channel: EventChannel) {
    let (event_sender, event_receiver) = main_channel;
    let slipmux_event_sender = create_slipmux_thread(event_sender.clone(), args.tty_path);

    match await_serial_connect(&event_receiver) {
        Ok(name) => {
            println!("Serial connect with {name}");
            create_raw_terminal_thread(event_sender.clone());
        }
        Err(e) => {
            println!("{e}");
            return;
        }
    }

    let name = args.network.flatten().unwrap_or_else(|| "slip".to_owned());
    let network_event_sender = create_network_thread(event_sender, &name);

    event_loop_diagnostic_network(
        &event_receiver,
        &slipmux_event_sender,
        &network_event_sender,
    );
}
