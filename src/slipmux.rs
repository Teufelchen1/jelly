use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;
use std::{thread, time};

use coap_lite::Packet;
use serial_line_ip::Decoder;
use serial_line_ip::Encoder;
use serialport::SerialPort;

use crate::events::Event;

const DIAGNOSTIC: u8 = 0x0a;
const CONFIGURATION: u8 = 0xA9;

pub fn create_slipmux_thread(sender: Sender<Event>, device_path: String) -> JoinHandle<()> {
    thread::spawn(move || read_thread(sender, device_path))
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

fn read_loop(mut read_port: Box<dyn SerialPort>, sender: &Sender<Event>) {
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

pub fn read_thread(sender: Sender<Event>, device_path: String) {
    //let path = device_path.to_str().unwrap();
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
        let _ = sender.send(Event::SerialConnect(port));
        read_loop(read_port, &sender);
    }
}
