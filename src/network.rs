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
    let (jelly_packet_sender, jelly_packet_receiver): (Sender<Event>, Receiver<Event>) =
        mpsc::channel();
    let (internal_packet_sender, internal_packet_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel();

    let Ok(dev) = DeviceBuilder::new()
        .name(name)
        .inherit_enable_state()
        .build_sync()
        .map_err(|err| match err.kind() {
            ErrorKind::ResourceBusy => {
                panic!("Network interface {name} is used by another program (possibly another Jelly instance); each running instance needs a dedicated interface.");
            }
            ErrorKind::PermissionDenied => {
                panic!("Not enough permissions to open network interface {name}. On Linux using NetworkManager, can create an interface with this command:\nsudo nmcli connection add type tun mode tun owner $(id -u) ifname {name} con-name {name} ipv6.method shared");
            }
            _ => {
                panic!("Error creating tun interface: {:}", err.kind());
            }
        });

    dev.set_nonblocking(true).unwrap();

    event_sender
        .send(Event::NetworkConnect(name.to_owned()))
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
