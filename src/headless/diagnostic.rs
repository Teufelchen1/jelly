use crate::Event;
use std::io;
use std::io::stdin;
use std::io::Write;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

fn raw_terminal_thread(sender: &Sender<Event>) {
    let mut buffer = String::new();
    loop {
        if let Ok(len) = stdin().read_line(&mut buffer) {
            if len == 0 {
                println!("Stdin reached EOF. You might continue to receive data from the device.");
                return;
            }
            sender.send(Event::SendDiagnostic(buffer.clone())).unwrap();
        }
        buffer.clear();
    }
}

fn create_raw_terminal_thread(sender: Sender<Event>) -> JoinHandle<()> {
    thread::spawn(move || raw_terminal_thread(&sender))
}

pub fn event_loop_diagnostic(
    event_channel: &Receiver<Event>,
    event_sender: Sender<Event>,
    hardware_event_sender: &Sender<Event>,
) {
    // The first event must be the serial connect. We wait for that event before processing
    // stdin. If we were to process stdin first, we might try to send data to the device
    // before it is even connected. This data would be lost.
    let event = match event_channel.recv_timeout(Duration::from_millis(5000)) {
        Ok(event) => event,
        Err(RecvTimeoutError::Timeout) => {
            println!("Timedout while waiting for serial to connect.");
            return;
        }
        Err(RecvTimeoutError::Disconnected) => panic!(),
    };
    if let Event::SerialConnect(name) = event {
        println!("Serial connect with {name}");
        create_raw_terminal_thread(event_sender);
    } else {
        println!("Unkown event occoured while waiting for serial to connect.");
        return;
    }

    loop {
        let event = match event_channel.recv_timeout(Duration::from_millis(5000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        match event {
            Event::Diagnostic(msg) => {
                print!("{msg}");
                io::stdout().flush().unwrap();
            }
            Event::SendDiagnostic(d) => hardware_event_sender
                .send(Event::SendDiagnostic(d))
                .unwrap(),
            Event::SerialDisconnect => {
                println!("\nSerial disconnect :(");
                return;
            }
            _ => (),
        }
    }
}
