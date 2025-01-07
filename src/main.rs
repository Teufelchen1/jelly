use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use events::{create_terminal_thread, event_loop, Event};
//use slipmux::read_thread;
//use tui::show;

use slipmux::create_slipmux_thread;
//use std::io::stdin;
//use std::io::BufRead;

mod app;
mod events;
mod slipmux;

fn reset_terminal() {
    crossterm::terminal::disable_raw_mode().unwrap();
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::cursor::Show
    )
    .unwrap();
}

fn main() {
    let (event_sender, event_receiver): (Sender<Event>, Receiver<Event>) = mpsc::channel();
    let slipmux_event_sender = event_sender.clone();
    let terminal_event_sender = event_sender.clone();
    let _ = create_slipmux_thread(slipmux_event_sender);
    let _ = create_terminal_thread(terminal_event_sender);

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        reset_terminal();
        original_hook(panic);
    }));

    crossterm::terminal::enable_raw_mode().unwrap();
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        crossterm::cursor::Hide
    )
    .unwrap();
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout)).unwrap();

    terminal.clear().unwrap();

    event_loop(event_receiver, terminal);

    reset_terminal();
    // let (diagnostic_tx, diagnostic_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    // let (configuration_tx, configuration_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
    //     mpsc::channel();
    // let (packet_tx, packet_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

    // //let conf_tx = configuration_tx.clone();

    // let mut port = serialport::new("/dev/ttyACM0", 115200)
    //     .open()
    //     .expect("Error");
    // let _ = port.set_timeout(Duration::from_secs(60));
    // let read_port = port.try_clone().unwrap();
    // let mut write_port = port.try_clone().unwrap();

    // // let _ =
    // //     thread::spawn(move || read_thread(read_port, diagnostic_tx, configuration_tx, packet_tx));

    // // loop {
    // //     let mut line = String::new();
    // //     {
    // //         println!("Reading: ");
    // //         stdin().lock().read_line(&mut line).unwrap();
    // //     }
    // //     let (data, size) = send_diagnostic(&line);
    // //     let _ = write_port.write(&data[..size]);
    // //     let _ = write_port.flush();
    // //     println!("Sending data..");
    // // }

    // let _ =
    //     thread::spawn(move || read_thread(read_port, diagnostic_tx, configuration_tx, packet_tx));
    // show(write_port, diagnostic_rx, configuration_rx, packet_rx);
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
