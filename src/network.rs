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

fn get_addr_as_string(addrs: std::io::Result<Vec<std::net::IpAddr>>) -> Option<String> {
    if let Ok(addrs) = addrs {
        for addr in addrs {
            if addr.is_ipv4() {
                return Some(addr.to_string());
            } else if addr.is_ipv6() {
                if addr.is_multicast() || addr.is_loopback() || addr.is_unspecified() {
                    continue;
                }
                return Some(addr.to_string());
            }
        }
    }
    None
}

pub fn open_network_device(name: &str) -> Result<SyncDevice, String> {
    fn match_err(err: std::io::Error, name: &str) -> String {
        match err.kind() {
            ErrorKind::ResourceBusy => {
                format!(
                    "Network interface {name} is used by another program (possibly another Jelly instance); each running instance needs a dedicated interface."
                )
            }
            ErrorKind::PermissionDenied => {
                format!(
                    "Not enough permissions to open network interface {name} or the device might not exist.\n"
                ) + "On Linux you can create a suitable interface with either of these commands:\n"
                    + "  Using NetworkManager:\n\t"
                    + &format!(
                        "sudo nmcli connection add type tun mode tun owner $(id -u) ifname {name} con-name {name} ipv6.method shared\n"
                    )
                    + "  Using ip tools:\n\t"
                    + &format!(
                        "sudo ip tuntap add {name} mode tun user $(id -u) && sudo ip link set up {name}"
                    )
            }
            ErrorKind::InvalidInput => {
                format!("Network interface {name} does not appear to be a TUN interface.")
            }
            _ => {
                format!("Error creating tun interface: {:}", err.kind())
            }
        }
    }
    let dev = DeviceBuilder::new()
        .name(name)
        .inherit_enable_state()
        .build_sync()
        .map_err(|err| match_err(err, name))?;

    dev.set_nonblocking(true)
        .map_err(|err| match_err(err, name))?;
    Ok(dev)
}

pub fn create_network_thread(
    event_sender: Sender<Event>,
    dev: SyncDevice,
    name: &str,
) -> Sender<Event> {
    let (jelly_packet_sender, jelly_packet_receiver): (Sender<Event>, Receiver<Event>) =
        mpsc::channel();
    let (internal_packet_sender, internal_packet_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel();

    let addr = get_addr_as_string(dev.addresses());

    let name = if let Some(addr) = addr {
        format!("{name} {addr}")
    } else {
        name.to_owned()
    };

    event_sender.send(Event::NetworkConnect(name)).unwrap();

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
