use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::Event;
use crate::app::App;

use super::await_serial_connect;
use super::create_raw_terminal_thread;

pub fn event_loop_configuration(
    event_channel: &Receiver<Event>,
    event_sender: Sender<Event>,
    hardware_event_sender: &Sender<Event>,
) {
    let mut app = App::new(event_sender.clone());
    app.force_all_commands_availabe(&mut None);

    match await_serial_connect(event_channel) {
        Ok(_name) => {
            app.on_connect();
            create_raw_terminal_thread(event_sender);
        }
        Err(e) => {
            println!("{e}");
            return;
        }
    }

    let mut terminal_eof = false;
    loop {
        let event = match event_channel.recv_timeout(Duration::from_millis(3000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => {
                if terminal_eof {
                    println!("\nTime-out, no response from device :(");
                    return;
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        match event {
            Event::TerminalString(msg) => {
                app.on_msg_string(None, &msg);
            }
            Event::TerminalEOF => {
                terminal_eof = true;
            }
            Event::Configuration(data) => {
                app.on_configuration_msg(None, &data);
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
        if terminal_eof && app.unfinished_jobs_count() == 0 {
            for msg in app.dump_job_log() {
                print!("{msg}");
            }
            return;
        }
    }
}
