use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::datatypes::packet_log::PacketLog;
use crate::Event;

pub fn event_loop_network(
    event_receiver: &Receiver<Event>,
    hardware_event_sender: &Sender<Event>,
    network_event_sender: &Sender<Event>,
) {
    loop {
        let event = match event_receiver.recv_timeout(Duration::from_millis(5000)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => panic!(),
        };
        match event {
            Event::NetworkConnect(name) => println!("Created network interface {name}."),

            Event::SerialDisconnect => {
                println!("\nSerial disconnect :(");
                return;
            }
            Event::Packet(packet) => {
                if let Ok(packet_direction) = PacketLog::packet_to_host(&packet) {
                    println!(
                        "{:} | {:}",
                        packet_direction.get_title(),
                        packet_direction.get_payload()
                    );
                }
                network_event_sender.send(Event::Packet(packet)).unwrap();
            }
            Event::SendPacket(packet) => {
                if let Ok(packet_direction) = PacketLog::packet_to_node(&packet) {
                    println!(
                        "{:} | {:}",
                        packet_direction.get_title(),
                        packet_direction.get_payload()
                    );
                }
                hardware_event_sender
                    .send(Event::SendPacket(packet))
                    .unwrap();
            }
            Event::TerminalEOF => {
                println!("Stdin reached EOF. You might continue to receive data from the device.");
            }
            _ => (),
        }
    }
}
