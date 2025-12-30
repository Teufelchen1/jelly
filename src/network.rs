use std::io::ErrorKind;
use std::io::ErrorKind::Interrupted;
use std::io::ErrorKind::TimedOut;
use std::io::ErrorKind::WouldBlock;
use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use tun_rs::DeviceBuilder;
use tun_rs::InterruptEvent;
use tun_rs::SyncDevice;

use crate::events::Event;

pub fn create_network_thread(event_sender: Sender<Event>, name: &str) -> Sender<Event> {
    // Ideally one would query what interfaces exist and then create only one
    // But for now, this silly work around will do
    fn create_interface(name: &str) -> (SyncDevice, String) {
        let mut index = 0;

        while index <= 9 {
            let name = format!("{name}{index}");
            match DeviceBuilder::new().name(&name).build_sync() {
                Ok(dev) => return (dev, name),
                Err(err) => match err.kind() {
                    ErrorKind::AlreadyExists => {
                        index += 1;
                    }
                    ErrorKind::PermissionDenied => {
                        panic!("Not enough permissions to create network interface");
                    }
                    _ => {
                        panic!("Error creating tun interface: {:}", err.kind());
                    }
                },
            }
        }
        panic!("could not create tun interface: too many interfaces");
    }
    let (jelly_packet_sender, jelly_packet_receiver): (Sender<Event>, Receiver<Event>) =
        mpsc::channel();
    let (internal_packet_sender, internal_packet_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel();

    let (dev, actual_name) = create_interface(name);
    dev.set_nonblocking(true).unwrap();

    event_sender
        .send(Event::NetworkConnect(actual_name))
        .unwrap();

    let interruptor = Arc::new(InterruptEvent::new().unwrap());
    let interruptor2 = interruptor.clone();

    thread::Builder::new()
        .name("NetworkWriterInterruptor".to_owned())
        .spawn(move || {
            write_thread(
                &jelly_packet_receiver,
                &internal_packet_sender,
                &interruptor,
            );
        })
        .unwrap();
    thread::Builder::new()
        .name("NetworkReaderWriter".to_owned())
        .spawn(move || {
            read_thread(
                &event_sender,
                &dev,
                &internal_packet_receiver,
                &interruptor2,
            );
        })
        .unwrap();
    jelly_packet_sender
}

fn write_thread(
    receiver: &Receiver<Event>,
    packet_sender: &Sender<Vec<u8>>,
    interruptor: &InterruptEvent,
) {
    while let Ok(event) = receiver.recv() {
        match event {
            Event::Packet(packet) => {
                let _: std::io::Result<()> = interruptor.trigger();
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
    tun_dev: &SyncDevice,
    packet_receiver: &Receiver<Vec<u8>>,
    interruptor: &InterruptEvent,
) {
    loop {
        let mut buf = vec![0; 65535].into_boxed_slice();
        let res = tun_dev.recv_intr(&mut buf, interruptor);
        let bytes_read = {
            match res {
                Ok(num) => {
                    if num == 0 {
                        sender
                            .send(Event::Diagnostic("Read zero bytes from tun\n".to_owned()))
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
                        }
                        continue;
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
