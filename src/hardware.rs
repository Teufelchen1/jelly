use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

use crate::slipmux::{SlipmuxDecoder, Slipmux};
use crate::events::Event;
use crate::transport::{SocketWrapper, SendPort};


pub fn create_slipmux_thread(sender: Sender<Event>, device_path: PathBuf) -> JoinHandle<()> {
    thread::spawn(move || read_thread(&sender, &device_path))
}

pub fn read_thread(sender: &Sender<Event>, device_path: &Path) {
    loop {
        let socket = SocketWrapper::new(device_path);
        let mut read_port = socket.clone_socket();
        let send_port = SendPort::new(Box::new(socket), device_path.to_string_lossy().to_string());
        sender
            .send(Event::SerialConnect(Box::new(send_port)))
            .unwrap();
        read_loop(&mut read_port, sender);
    }
    // loop {
    //     let mut port = match serialport::new(&device_path, 115200).open() {
    //         Ok(p) => p,
    //         Err(_) => {
    //             thread::sleep(Duration::from_millis(100));
    //             continue;
    //         }
    //     };
    //     let _ = port.set_timeout(Duration::from_secs(600));
    //     let read_port = port.try_clone().unwrap();
    //     let send_port = SendPort::new(SerialPortWrapper::new(port), device_path.clone());
    //     let _ = sender.send(Event::SerialConnect(Box::new(send_port)));
    //     read_loop(read_port, &sender);
    // }
}


fn read_loop(read_port: &mut impl Read, sender: &Sender<Event>) {
    let mut slipmux_decoder = SlipmuxDecoder::new();

    loop {
        let mut buffer = [0; 10240];
        let res = read_port.read(&mut buffer);
        let bytes_read = {
            match res {
                Ok(num) => num,
                Err(_) => {
                    // TODO: Catch timeout
                    sender.send(Event::SerialDisconnect).unwrap();
                    break;
                }
            }
        };
        sender.send(Event::Diagnostic(format!("Read {bytes_read} bytes\n"))).unwrap();

        for slipframe in slipmux_decoder.decode(&buffer[..bytes_read]) {
            match slipframe.unwrap() {
                Slipmux::Diagnostic(s) => {
                    sender.send(Event::Diagnostic(s)).unwrap();
                }
                Slipmux::Configuration(conf) => {
                    sender.send(Event::Configuration(conf)).unwrap();
                }
                Slipmux::Packet(packet) => {
                    sender.send(Event::Packet(packet)).unwrap();
                }
            }
        }
    }
}