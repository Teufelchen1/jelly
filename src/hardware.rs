use std::io::ErrorKind::WouldBlock;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use slipmux::encode_diagnostic;
use slipmux::Decoder;
use slipmux::Slipmux;

use crate::events::Event;
use crate::transport::new_port;
use crate::transport::ReaderWriter;

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

fn write_thread(receiver: Receiver<Event>, port_guard: Arc<Mutex<Option<impl Write>>>) {
    loop {
        match receiver.recv() {
            Ok(event) => match event {
                Event::SendDiagnostic(msg) => {
                    let (data, size) = encode_diagnostic(&msg);
                    let mut write_port = port_guard.lock().unwrap();

                    if let Some(port) = (*write_port).as_mut() {
                        let _ = port.write_all(&data[..size]);
                        let _ = port.flush();
                    } else {
                        // Nothing to do, drop the message silently
                        continue;
                    }
                }
                Event::SendConfiguration(conf) => {
                    let mut write_port = port_guard.lock().unwrap();
                    if let Some(port) = (*write_port).as_mut() {
                        let _ = port.write_all(&conf);
                        let _ = port.flush();
                    } else {
                        // Nothing to do, drop the message silently
                        continue;
                    }
                    thread::sleep(Duration::from_millis(100));
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
    port_guard: Arc<Mutex<Option<Box<dyn ReaderWriter>>>>,
) {
    loop {
        let (mut read_port, write_port) = match new_port(&device_path) {
            Ok((r, w)) => (r, w),
            Err(_) => {
                thread::sleep(Duration::from_millis(2500));
                continue;
            }
        };

        {
            let mut write_port_lock = port_guard.lock().unwrap();
            *write_port_lock = Some(write_port);
        }
        sender
            .send(Event::SerialConnect(
                device_path.to_string_lossy().to_string(),
            ))
            .unwrap();
        read_loop(&mut read_port, sender);

        {
            let mut write_port_lock = port_guard.lock().unwrap();
            *write_port_lock = None;
        }
        sender
            .send(Event::Diagnostic("Port died, waiting 3s\n".to_owned()))
            .unwrap();
        thread::sleep(Duration::from_millis(3000));
    }
}

fn read_loop(read_port: &mut impl Read, sender: &Sender<Event>) {
    let mut slipmux_decoder = Decoder::new();

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
                        // This means timeout
                        continue;
                    }
                    _ => {
                        sender.send(Event::SerialDisconnect).unwrap();
                        break;
                    }
                },
            }
        };

        for slipframe in slipmux_decoder.decode(&buffer[..bytes_read]) {
            if slipframe.is_err() {
                sender
                    .send(Event::Diagnostic("Received garbage\n".to_owned()))
                    .unwrap();
                continue;
            }
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
