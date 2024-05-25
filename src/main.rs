use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use slipmux::read_thread;
use tui::show;

mod slipmux;
mod tui;

fn main() {
    let (diagnostic_tx, diagnostic_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (configuration_tx, configuration_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel();
    let (packet_tx, packet_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

    //let conf_tx = configuration_tx.clone();

    let mut port = serialport::new("/dev/ttyACM1", 115200)
        .open()
        .expect("Error");
    let _ = port.set_timeout(Duration::from_secs(60));
    let read_port = port.try_clone().unwrap();
    let write_port = port.try_clone().unwrap();

    let _ =
        thread::spawn(move || read_thread(read_port, diagnostic_tx, configuration_tx, packet_tx));
    show(write_port, diagnostic_rx, configuration_rx, packet_rx);
    //let ui_loop =
    //    thread::spawn(move || print_thread(write_port, diagnostic_rx, configuration_rx, packet_rx));

    // let (data, size) = send_diagnostic("help\n");
    // let _ = port.write(&data[..size]);
    // let _ = port.flush();

    //let mut request: CoapRequest<String> = CoapRequest::new();

    // request.set_method(Method::Get);
    // request.set_path("/.well-known/core");
    // request.message.add_option(CoapOption::Block2, vec![0x05]);
    // conf_tx.send(request.message.to_bytes().unwrap()).unwrap();
    // let (data, size) = send_configuration(&request.message);
    // let _ = port.write(&data[..size]);
    // let _ = port.flush();
    // request.set_method(Method::Get);
    // request.set_path("version");
    // request.message.add_option(CoapOption::Block2, vec![0x05]);
    // conf_tx.send(request.message.to_bytes().unwrap()).unwrap();
    // let (data, size) = send_configuration(&request.message);
    // let _ = port.write(&data[..size]);
    // let _ = port.flush();
    //ui_loop.join().unwrap();

    // loop {
    //     let mut line = String::new();
    //     {
    //         stdin().lock().read_line(&mut line).unwrap();
    //     }
    //     let (data, size) = send_diagnostic(&line);
    //     let _ = port.write(&data[..size]);
    //     let _ = port.flush();
    // }
}
