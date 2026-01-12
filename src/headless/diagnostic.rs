use std::io::Write;
use std::io::stdout;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::Event;

pub fn event_loop_diagnostic(
    event_channel: &Receiver<Event>,
    hardware_event_sender: &Sender<Event>,
) {
    loop {
        let event = match event_channel.recv_timeout(Duration::from_millis(5000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        match event {
            Event::Diagnostic(msg) => {
                print!("{msg}");
                stdout().flush().unwrap();
            }
            Event::TerminalString(msg) => hardware_event_sender
                .send(Event::SendDiagnostic(msg))
                .unwrap(),
            Event::SerialDisconnect => {
                println!("\nSerial disconnect :(");
                return;
            }
            Event::TerminalEOF => {
                println!("Stdin reached EOF. You might continue to receive data from the device.");
            }
            _ => (),
        }
    }
}
