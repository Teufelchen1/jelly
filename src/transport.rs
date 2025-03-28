use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;

use serialport::SerialPort;

pub trait Transmit {
    fn transmit(&mut self, data: &[u8]) -> std::io::Result<()>;
}

pub struct SendPort {
    tx: Box<dyn Transmit + Send>,
    name: String,
}

impl SendPort {
    pub fn new(tx: Box<dyn Transmit + Send>, name: String) -> Self {
        Self { tx, name }
    }

    pub const fn name(&self) -> &String {
        &self.name
    }

    pub fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.tx.transmit(data)
    }
}

struct SerialPortWrapper {
    port: Box<dyn SerialPort>,
}

impl SerialPortWrapper {
    pub fn new(port: Box<dyn SerialPort>) -> Self {
        Self { port }
    }
}

impl Transmit for SerialPortWrapper {
    fn transmit(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.port.write_all(data)
    }
}

pub struct SocketWrapper {
    socket: UnixStream,
}

impl SocketWrapper {
    pub fn new(socket_path: &Path) -> Self {
        let socket = match UnixStream::connect(socket_path) {
            Ok(s) => s,
            Err(e) => panic!("{}", e),
        };
        Self { socket }
    }

    pub fn clone_socket(&self) -> UnixStream {
        self.socket.try_clone().unwrap()
    }
}

impl Transmit for SocketWrapper {
    fn transmit(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.socket.write_all(data)?;
        self.socket.flush()
    }
}
