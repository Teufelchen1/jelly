use std::fs::File;
use std::fs::OpenOptions;
use std::io::ErrorKind::WouldBlock;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::events::Event;
use crate::slipmux::{send_diagnostic, Slipmux, SlipmuxDecoder};
use crate::transport::{SendPort, SocketWrapper};

pub fn create_slipmux_thread(
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    device_path: PathBuf,
) -> JoinHandle<()> {
    let port_guard = Arc::new(Mutex::new(None));
    let cloned_port_guard = Arc::clone(&port_guard);
    thread::Builder::new()
        .name("HardwareWriter".to_owned())
        .spawn(move || write_thread(receiver, cloned_port_guard))
        .unwrap();
    thread::Builder::new()
        .name("HardwareReader".to_owned())
        .spawn(move || read_thread(&sender, &device_path, port_guard))
        .unwrap()
}

fn write_thread(receiver: Receiver<Event>, port_guard: Arc<Mutex<Option<SendPort>>>) {
    loop {
        match receiver.recv() {
            Ok(event) => match event {
                Event::SendDiagnostic(msg) => {
                    let (data, size) = send_diagnostic(&msg);
                    let mut write_port = port_guard.lock().unwrap();

                    if let Some(port) = (*write_port).as_mut() {
                        let _ = port.send(&data[..size]);

                        // let _ = port.flush();
                    } else {
                        // Nothing to do, drop the message silently
                        continue;
                    }
                }
                Event::SendConfiguration(conf) => {
                    let mut write_port = port_guard.lock().unwrap();
                    if let Some(port) = (*write_port).as_mut() {
                        let _ = port.send(&conf);
                        // let _ = port.flush();
                    } else {
                        // Nothing to do, drop the message silently
                        continue;
                    }
                }
                _ => todo!(),
            },
            Err(_) => break,
        }
    }
}

fn read_thread(
    sender: &Sender<Event>,
    device_path: &Path,
    port_guard: Arc<Mutex<Option<SendPort>>>,
) {
    loop {
        let socket = match SocketWrapper::new(device_path) {
            Ok(s) => s,
            Err(_) => {
                thread::sleep(Duration::from_millis(2500));
                continue;
            }
        };
        let mut read_port = socket.clone_socket();
        let send_port = SendPort::new(Box::new(socket), device_path.to_string_lossy().to_string());
        {
            let mut write_port = port_guard.lock().unwrap();
            *write_port = Some(send_port);
        }
        sender
            .send(Event::SerialConnect(
                device_path.to_string_lossy().to_string(),
            ))
            .unwrap();
        read_loop(&mut read_port, sender);

        {
            let mut write_port = port_guard.lock().unwrap();
            *write_port = None;
        }
        sender
            .send(Event::Diagnostic("Port died, waiting 3s\n".to_owned()))
            .unwrap();
        thread::sleep(Duration::from_millis(3000));
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
                Ok(num) => {
                    // Returning zero bytes means reaching end-of-file
                    if num == 0 {
                        sender.send(Event::SerialDisconnect).unwrap();
                        break;
                    }
                    num
                }
                Err(err) => match err.kind() {
                    WouldBlock => {
                        sender
                            .send(Event::Diagnostic("Time out?\n".to_owned()))
                            .unwrap();
                        continue;
                    }
                    _ => {
                        //let errkind = err.kind();
                        //panic!("{errkind}");
                        // TODO: Catch timeout
                        sender.send(Event::SerialDisconnect).unwrap();
                        break;
                    }
                },
            }
        };

        sender
            .send(Event::Diagnostic(format!("Read {bytes_read} bytes\n")))
            .unwrap();

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
