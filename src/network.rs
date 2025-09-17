use std::io::ErrorKind::Interrupted;
use std::io::ErrorKind::TimedOut;
use std::io::ErrorKind::WouldBlock;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

use tun_rs::InterruptEvent;
use tun_rs::SyncDevice;

use crate::events::Event;

pub fn create_network_thread(event_sender: Sender<Event>) -> Sender<Event> {
    let (slipmux_sender, slipmux_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();
    let (packet_sender, packet_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

    use tun_rs::DeviceBuilder;
    let dev = DeviceBuilder::new()
        .name("tun1")
        .ipv6("fe80::acab", 64)
        .multi_queue(true)
        .build_sync()
        .unwrap();

    let interruptor = Arc::new(InterruptEvent::new().unwrap());
    let interruptor2 = interruptor.clone();

    thread::Builder::new()
        .name("NetworkWriter".to_owned())
        .spawn(move || write_thread(&slipmux_receiver, &packet_sender, &interruptor))
        .unwrap();
    thread::Builder::new()
        .name("NetworkReader".to_owned())
        .spawn(move || read_thread(&event_sender, dev, &packet_receiver, &interruptor2))
        .unwrap();
    slipmux_sender
}

fn write_thread(
    receiver: &Receiver<Event>,
    packet_sender: &Sender<Vec<u8>>,
    interruptor: &InterruptEvent,
) {
    while let Ok(event) = receiver.recv() {
        match event {
            Event::Packet(packet) => {
                let _ = interruptor.trigger();
                packet_sender.send(packet.clone()).unwrap();
            }
            _ => {
                println!("Something went wrong");
            }
        }
    }
}

fn read_thread(
    sender: &Sender<Event>,
    tun_dev: SyncDevice,
    packet_receiver: &Receiver<Vec<u8>>,
    interruptor: &InterruptEvent,
) {
    loop {
        let mut buf = [0; 65535];
        let _ = tun_dev.set_nonblocking(true);
        let res = tun_dev.recv_intr(&mut buf, &interruptor);
        let bytes_read = {
            match res {
                Ok(num) => {
                    if num == 0 {
                        sender
                            .send(Event::Diagnostic(format!("Read zero bytes from tun\n")))
                            .unwrap();
                        break;
                    }
                    num
                }
                Err(err) => {
                    // WouldBlock is timeout on unix sockets
                    if err.kind() == WouldBlock || err.kind() == TimedOut {
                        continue;
                    }
                    if err.kind() == Interrupted {
                        if interruptor.is_trigger() {
                            interruptor.reset().unwrap();
                            if let Ok(packet) = packet_receiver.try_recv() {
                                tun_dev.send(&packet).unwrap();
                            }

                            continue;
                        } else {
                            continue;
                        }
                    }
                    sender
                        .send(Event::Diagnostic(format!("Tun Port error {err:?}")))
                        .unwrap();
                    sender.send(Event::SerialDisconnect).unwrap();
                    break;
                }
            }
        };
        sender
            .send(Event::SendPacket(buf[..bytes_read].to_vec()))
            .unwrap();
    }
}
