use std::fs::File;
use std::io::{Error, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;
use std::{thread, time};

use coap_lite::Packet;
use serial_line_ip::Decoder;
use serial_line_ip::Encoder;
use serialport::{SerialPort, TTYPort};

use crate::events::Event;

const DIAGNOSTIC: u8 = 0x0a;
const CONFIGURATION: u8 = 0xA9;

pub trait Transmit {
    fn transmit(&mut self, data: &[u8]) -> Result<(), Error>;
}

pub struct SendPort {
    tx: Box<dyn Transmit + Send>,
    name: String,
}

impl SendPort {
    pub fn new(tx: Box<dyn Transmit + Send>, name: String) -> Self {
        Self { tx, name }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.tx.transmit(data)
    }
}

struct SerialPortWrapper {
    port: Box<dyn SerialPort>,
}

impl SerialPortWrapper {
    pub fn new(port: Box<dyn SerialPort>) -> Box<Self> {
        Box::new(Self { port })
    }
}

impl Transmit for SerialPortWrapper {
    fn transmit(&mut self, data: &[u8]) -> Result<(), Error> {
        self.port.write_all(data)
    }
}

struct SocketWrapper {
    socket: UnixStream,
}

impl SocketWrapper {
    pub fn new(socket_path: String) -> Box<Self> {
        let socket = match UnixStream::connect(socket_path) {
            Ok(s) => s,
            Err(e) => panic!("{}", e),
        };
        Box::new(Self { socket })
    }

    pub fn clone_socket(&self) -> UnixStream {
        self.socket.try_clone().unwrap()
    }
}

impl Transmit for SocketWrapper {
    fn transmit(&mut self, data: &[u8]) -> Result<(), Error> {
        let err = self.socket.write_all(data);
        self.socket.flush();
        err
    }
}

pub fn create_slipmux_thread(sender: Sender<Event>, device_path: String) -> JoinHandle<()> {
    thread::spawn(move || read_thread(sender, device_path))
}

pub fn read_thread(sender: Sender<Event>, device_path: String) {
    loop {
        let socket = SocketWrapper::new(device_path.clone());
        let read_port = socket.clone_socket();
        let send_port = SendPort::new(socket, device_path.clone());
        let _ = sender.send(Event::SerialConnect(Box::new(send_port)));
        read_loop(read_port, &sender);
    }
    loop {
        let mut port = match serialport::new(&device_path, 115200).open() {
            Ok(p) => p,
            Err(_) => {
                thread::sleep(time::Duration::from_millis(100));
                continue;
            }
        };
        let _ = port.set_timeout(Duration::from_secs(600));
        let read_port = port.try_clone().unwrap();
        let send_port = SendPort::new(SerialPortWrapper::new(port), device_path.clone());
        let _ = sender.send(Event::SerialConnect(Box::new(send_port)));
        read_loop(read_port, &sender);
    }
}

pub fn send_diagnostic(text: &str) -> ([u8; 256], usize) {
    let mut output: [u8; 256] = [0; 256];
    let mut slip = Encoder::new();
    let mut totals = slip.encode(&[DIAGNOSTIC], &mut output).unwrap();
    totals += slip
        .encode(text.as_bytes(), &mut output[totals.written..])
        .unwrap();
    totals += slip.finish(&mut output[totals.written..]).unwrap();
    (output, totals.written)
}

pub fn send_configuration(packet: &Packet) -> ([u8; 256], usize) {
    let mut output: [u8; 256] = [0; 256];
    let mut slip = Encoder::new();
    let mut totals = slip.encode(&[CONFIGURATION], &mut output).unwrap();
    totals += slip
        .encode(&packet.to_bytes().unwrap(), &mut output[totals.written..])
        .unwrap();
    totals += slip.finish(&mut output[totals.written..]).unwrap();
    (output, totals.written)
}

fn read_loop(mut read_port: impl Read, sender: &Sender<Event>) {
    let mut slip_decoder = Decoder::new();
    let mut strbuffer = String::new();
    let mut output = [0; 2024];
    let mut index = 0;
    let _ = slip_decoder.decode(&[0xc0], &mut output);
    loop {
        let mut buffer = [0; 1024];
        let mut offset = 0;
        let res = read_port.read(&mut buffer);
        let num = {
            match res {
                Ok(num) => num,
                Err(_) => {
                    // TODO: Catch timeout
                    let _ = sender.send(Event::SerialDisconnect);
                    break;
                }
            }
        };
        'inner: while offset < num {
            let (used, out, end) = {
                match slip_decoder.decode(&buffer[offset..num], &mut output[index..]) {
                    Ok((used, out, end)) => (used, out, end),
                    Err(_) => {
                        break 'inner;
                    }
                }
            };
            index += out.len();
            offset += used;

            if end {
                match output[0] {
                    DIAGNOSTIC => {
                        let s = String::from_utf8_lossy(&output[1..index]).to_string();
                        if s.contains('\n') {
                            strbuffer.push_str(&s);
                            let _ = sender.send(Event::Diagnostic(strbuffer.clone()));
                            strbuffer.clear();
                        } else {
                            strbuffer.push_str(&s);
                        }
                        // let _ = sender.send(Event::Diagnostic(
                        //     String::from_utf8_lossy(&output[1..index]).to_string(),
                        // ));
                    }
                    CONFIGURATION => {
                        let _ = sender.send(Event::Configuration(output[1..index].to_vec()));
                    }
                    _ => {
                        let _ = sender.send(Event::Packet(output[0..index].to_vec()));
                    }
                }
                slip_decoder = Decoder::new();
                let _ = slip_decoder.decode(&[0xc0], &mut output);
                output = [0; 2024];
                index = 0;
            }
        }
    }
}
