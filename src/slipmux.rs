use coap_lite::Packet;
use serial_line_ip::Decoder;
use serial_line_ip::EncodeTotals;
use serial_line_ip::Encoder;
use serial_line_ip::Error;

const DIAGNOSTIC: u8 = 0x0a;
const CONFIGURATION: u8 = 0xA9;

pub fn send_diagnostic(text: &str) -> ([u8; 256], usize) {
    encode(Slipmux::Diagnostic(text.to_string()))
}

pub fn send_configuration(packet: &Packet) -> ([u8; 256], usize) {
    encode(Slipmux::Configuration(packet.to_bytes().unwrap()))
}

pub fn encode(input: Slipmux) -> ([u8; 256], usize) {
    let mut buffer = [0; 256];
    let mut slip = Encoder::new();
    let mut totals = EncodeTotals {
        read: 0,
        written: 0,
    };
    match input {
        Slipmux::Diagnostic(s) => {
            totals += slip.encode(&[DIAGNOSTIC], &mut buffer).unwrap();
            totals += slip
                .encode(s.as_bytes(), &mut buffer[totals.written..])
                .unwrap();
        }
        Slipmux::Configuration(conf) => {
            totals += slip.encode(&[CONFIGURATION], &mut buffer).unwrap();
            totals += slip.encode(&conf, &mut buffer[totals.written..]).unwrap();
        }
        Slipmux::Packet(packet) => {
            totals += slip.encode(&packet, &mut buffer[totals.written..]).unwrap();
        }
    }
    totals += slip.finish(&mut buffer[totals.written..]).unwrap();
    (buffer, totals.written)
}

pub enum Slipmux {
    Diagnostic(String),
    Configuration(Vec<u8>),
    Packet(Vec<u8>),
}

enum SlipmuxState {
    Fin(Result<Slipmux, Error>, usize),
    Error(Error),
    Incomplete(),
}

pub struct SlipmuxDecoder {
    slip_decoder: Decoder,
    index: usize,
    buffer: [u8; 10240],
}

impl SlipmuxDecoder {
    pub fn new() -> Self {
        Self {
            slip_decoder: Decoder::new(),
            index: 0,
            buffer: [0; 10240],
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Vec<Result<Slipmux, Error>> {
        let mut result_vec = Vec::new();
        let mut offset = 0;
        while offset < input.len() {
            let used_bytes = {
                match self.decode_partial(&input[offset..input.len()]) {
                    SlipmuxState::Fin(data, bytes_consumed) => {
                        result_vec.push(data);
                        bytes_consumed
                    }
                    SlipmuxState::Error(err) => {
                        result_vec.push(Err(err));
                        break;
                    }
                    SlipmuxState::Incomplete() => input.len(),
                }
            };
            offset += used_bytes;
        }
        result_vec
    }

    fn decode_partial(&mut self, input: &[u8]) -> SlipmuxState {
        let partial_result = self
            .slip_decoder
            .decode(input, &mut self.buffer[self.index..]);
        if partial_result.is_err() {
            return SlipmuxState::Error(partial_result.unwrap_err());
        }
        let (used_bytes_from_input, out, end) = partial_result.unwrap();
        self.index += out.len();

        if end {
            let retval = {
                match self.buffer[0] {
                    DIAGNOSTIC => {
                        let s = String::from_utf8_lossy(&self.buffer[1..self.index]).to_string();
                        Ok(Slipmux::Diagnostic(s))
                    }
                    CONFIGURATION => {
                        Ok(Slipmux::Configuration(self.buffer[1..self.index].to_vec()))
                    }
                    _ => Ok(Slipmux::Packet(self.buffer[1..self.index].to_vec())),
                }
            };

            self.slip_decoder = Decoder::new();
            self.index = 0;
            SlipmuxState::Fin(retval, used_bytes_from_input)
        } else {
            assert!(used_bytes_from_input == input.len());
            SlipmuxState::Incomplete()
        }
    }
}
