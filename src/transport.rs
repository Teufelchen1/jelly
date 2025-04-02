use std::io::Error;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serialport::SerialPort;

pub trait ReaderWriter: Read + Write + Send {}

impl ReaderWriter for Box<dyn SerialPort> {}
impl ReaderWriter for UnixStream {}

pub fn new_port(
    device_path: &Path,
) -> Result<(Box<dyn ReaderWriter>, Box<dyn ReaderWriter>), Error> {
    let file_type = device_path.metadata()?.file_type();
    let (read_writeable0, read_writeable1): (Box<dyn ReaderWriter>, Box<dyn ReaderWriter>) =
        if file_type.is_char_device() {
            let mut port = serialport::new(device_path.to_string_lossy(), 115200).open()?;
            let _ = port.set_timeout(Duration::from_secs(600));
            (Box::new(port.try_clone().unwrap()), Box::new(port))
        } else if file_type.is_socket() {
            let socket = UnixStream::connect(&device_path)?;
            socket.set_read_timeout(Some(Duration::new(1, 0))).unwrap();
            socket.set_write_timeout(Some(Duration::new(1, 0))).unwrap();
            (Box::new(socket.try_clone().unwrap()), Box::new(socket))
        } else {
            panic!();
        };
    Ok((read_writeable0, read_writeable1))
}
