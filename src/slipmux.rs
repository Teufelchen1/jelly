use std::sync::mpsc::Sender;

use coap_lite::Packet;
use serial_line_ip::Decoder;
use serial_line_ip::Encoder;
use serialport::SerialPort;

const DIAGNOSTIC: u8 = 0x0a;
const CONFIGURATION: u8 = 0xA9;

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

pub fn read_thread(
    mut read_port: Box<dyn SerialPort>,
    diagnostic_channel: Sender<String>,
    configuration_channel: Sender<Vec<u8>>,
    packet_channel: Sender<Vec<u8>>,
) {
    let mut slip_decoder = Decoder::new();
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
                    continue;
                }
            }
        };
        while offset < num {
            let (used, out, end) = {
                match slip_decoder.decode(&buffer[offset..num], &mut output[index..]) {
                    Ok((used, out, end)) => (used, out, end),
                    Err(_) => {
                        break;
                    }
                }
            };
            index += out.len();
            offset += used;

            if end {
                match output[0] {
                    DIAGNOSTIC => {
                        let _ = diagnostic_channel
                            .send(String::from_utf8_lossy(&output[1..index]).to_string());
                    }
                    CONFIGURATION => {
                        let _ = configuration_channel.send(output[1..index].to_vec());
                    }
                    _ => {
                        let _ = packet_channel.send(output[0..index].to_vec());
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
