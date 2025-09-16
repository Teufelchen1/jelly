use std::io::ErrorKind::TimedOut;
use std::io::ErrorKind::WouldBlock;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use tun_rs::SyncDevice;

use crate::events::Event;

pub fn create_network_thread(event_sender: Sender<Event>) -> Sender<Event> {
    let (slipmux_sender, slipmux_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();

    use tun_rs::DeviceBuilder;
    let dev = DeviceBuilder::new()
        .name("utun7")
        .ipv6("fe80::acab", 64)
        .mtu(1400)
        .multi_queue(true)
        .build_sync()
        .unwrap();

    let dev2 = dev.try_clone().unwrap();
    thread::Builder::new()
        .name("NetworkWriter".to_owned())
        .spawn(move || write_thread(&slipmux_receiver, dev2))
        .unwrap();
    thread::Builder::new()
        .name("NetworkReader".to_owned())
        .spawn(move || read_thread(&event_sender, dev))
        .unwrap();
    slipmux_sender
}

fn write_thread(receiver: &Receiver<Event>, tun_dev: SyncDevice) {
    while let Ok(event) = receiver.recv() {
        match event {
            Event::Packet(packet) => {
                println!("<- Send packet to host {:}", packet.len());
                tun_dev.send(&packet).unwrap();
            }
            _ => {
                println!("Something went wrong");
            }
        }
    }
}

fn read_thread(sender: &Sender<Event>, tun_dev: SyncDevice) {
    loop {
        let mut buf = [0; 65535];
        let res = tun_dev.recv(&mut buf);
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
                    sender
                        .send(Event::Diagnostic(format!("Tun Port error {err:?}\n")))
                        .unwrap();
                    sender.send(Event::SerialDisconnect).unwrap();
                    break;
                }
            }
        };
        println!(
            "-> Got packet from host {bytes_read} bytes\n{:?}",
            &buf[..bytes_read]
        );
        sender
            .send(Event::SendPacket(buf[..bytes_read].to_vec()))
            .unwrap();
    }
}
