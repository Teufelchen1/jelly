use crate::app::App;
use crate::Event;
use std::io::stdin;
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

pub fn event_loop_configuration(
    event_channel: &Receiver<Event>,
    event_sender: Sender<Event>,
    hardware_event_sender: &Sender<Event>,
) {
    let mut app = App::new(event_sender.clone());
    app.force_all_commands_availabe();
    // The first event must be the serial connect. We wait for that event before processing
    // stdin. If we were to process stdin first, we might try to send data to the device
    // before it is even connected. This data would be lost.
    let event = match event_channel.recv_timeout(Duration::from_millis(5000)) {
        Ok(event) => event,
        Err(RecvTimeoutError::Timeout) => {
            println!("Time-out while waiting for serial to connect.");
            return;
        }
        Err(RecvTimeoutError::Disconnected) => panic!(),
    };
    if let Event::SerialConnect(name) = event {
        app.on_connect(name);
    } else {
        println!("Unkown event occoured while waiting for serial to connect.");
        return;
    }

    create_raw_terminal_thread(event_sender);

    loop {
        let event = match event_channel.recv_timeout(Duration::from_millis(3000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => {
                println!("\nTime-out, no response from device :(");
                return;
            }
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        match event {
            Event::TerminalString(msg) => {
                app.on_msg_string(&msg);
            }
            Event::Configuration(data) => {
                app.on_configuration_msg(&data);
                if app.unfinished_jobs_count() == 0 {
                    for msg in app.dump_job_log() {
                        print!("{msg}");
                    }
                    return;
                }
            }
            Event::SendConfiguration(c) => hardware_event_sender
                .send(Event::SendConfiguration(c))
                .unwrap(),
            Event::SerialDisconnect => {
                println!("\nSerial disconnect :(");
                return;
            }
            _ => (),
        }
    }
}
