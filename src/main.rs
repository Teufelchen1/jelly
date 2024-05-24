use coap_lite::CoapOption;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::{CoapRequest, Packet, RequestType as Method};
use crossterm::event::Event;
use crossterm::event::KeyCode;
use ratatui::prelude::Alignment;
use ratatui::prelude::Constraint;
use ratatui::prelude::CrosstermBackend;
use ratatui::prelude::Direction;
use ratatui::prelude::Layout;
use ratatui::prelude::Span;
use ratatui::prelude::Text;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui::Terminal;
use serial_line_ip::{Decoder, Encoder};
use serialport::SerialPort;
use std::time::Instant;

use std::io::{Read, Write};

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

enum Refresh {
    /// Update the TUI
    Update,
    /// Skip the update of the TUI
    Skip,
    /// Quit the TUI and return to the shell
    Quit,
}

const DIAGNOSTIC: u8 = 0x0a;
const CONFIGURATION: u8 = 0xA9;

fn send_diagnostic(text: &str) -> ([u8; 256], usize) {
    let mut output: [u8; 256] = [0; 256];
    let mut slip = Encoder::new();
    let mut totals = slip.encode(&[DIAGNOSTIC], &mut output).unwrap();
    totals += slip
        .encode(text.as_bytes(), &mut output[totals.written..])
        .unwrap();
    totals += slip.finish(&mut output[totals.written..]).unwrap();
    return (output, totals.written);
}

fn send_configuration(packet: &Packet) -> ([u8; 256], usize) {
    let mut output: [u8; 256] = [0; 256];
    let mut slip = Encoder::new();
    let mut totals = slip.encode(&[CONFIGURATION], &mut output).unwrap();
    totals += slip
        .encode(&packet.to_bytes().unwrap(), &mut output[totals.written..])
        .unwrap();
    totals += slip.finish(&mut output[totals.written..]).unwrap();
    return (output, totals.written);
}

fn read_thread(
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
                    //println!("{:}", e);
                    continue;
                }
            }
        };

        while offset < num {
            let (used, out, end) = {
                match slip_decoder.decode(&buffer[offset..num], &mut output[index..]) {
                    Ok((used, out, end)) => (used, out, end),
                    Err(_) => {
                        //println!("{:}", e);
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

fn fmt_packet(packet: &Packet) -> String {
    let mut out: String = Default::default();

    let class = {
        match packet.header.code {
            MessageClass::Empty => "Empty ",
            MessageClass::Request(rtype) => {
                let payload: String = "Empty Payload".to_string();
                let contentformat = {
                    match packet.get_first_option(CoapOption::UriPath) {
                        Some(ref cf) => &format!("{:}", &String::from_utf8_lossy(&cf)),
                        None => "",
                    }
                };
                &format!("<- Req({:?}; {:})\n  {:}", rtype, contentformat, payload)
            }
            MessageClass::Response(rtype) => {
                let mut payload: String = "Empty Payload".to_string();
                let contentformat = {
                    match packet.get_content_format() {
                        Some(cf) => {
                            match cf {
                                ContentFormat::ApplicationLinkFormat => {
                                    payload = String::from_utf8_lossy(&packet.payload)
                                        .to_string()
                                        .replace(",", "\n  ");
                                }
                                ContentFormat::TextPlain => {
                                    payload = String::from_utf8_lossy(&packet.payload)
                                        .to_string()
                                        .replace(",", "\n  ");
                                }
                                _ => todo!(),
                            }
                            &format!("{:?}", cf)
                        }
                        None => "",
                    }
                };
                &format!("-> Res({:?}/{:})\n  {:}", rtype, contentformat, payload)
            }
            MessageClass::Reserved(_) => "Reserved ",
        }
    };
    out.push_str(class);
    //out.push_str(&format!("{:04x} ", packet.header.message_id));
    out
}

fn draw(
    frame: &mut Frame,
    diagnostic_messages: &String,
    user_command: &String,
    configuration_packets: &Vec<Packet>,
) {
    let size = frame.size();

    let block = Block::bordered()
        .title("Main")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Rounded);
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(frame.size());

    let horizontal_chunk_left = horizontal_chunks[0];
    let horizontal_chunk_right = horizontal_chunks[1];

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
        .split(horizontal_chunk_right);

    let right_chunk_upper = right_chunks[0];
    let right_chunk_lower = right_chunks[1];

    let right_block_up = Block::bordered()
        .title(vec![Span::from("Diagnostic Messages")])
        .title_alignment(Alignment::Left);

    let right_block_down = Block::bordered()
        .title(vec![Span::from("User Input")])
        .title_alignment(Alignment::Left);

    let text: &str = &diagnostic_messages;
    let text = Text::from(text);
    let paragraph = Paragraph::new(text).block(right_block_up);
    frame.render_widget(paragraph, right_chunk_upper);

    let text: &str = &user_command;
    let text = Text::from(text);
    let paragraph = Paragraph::new(text).block(right_block_down);
    frame.render_widget(paragraph, right_chunk_lower);

    let left_block = Block::bordered()
        .title(vec![Span::from("Configuration Packets")])
        .title_alignment(Alignment::Left);
    let items: Vec<ListItem> = configuration_packets
        .iter()
        .map(|i| ListItem::new(fmt_packet(i)))
        .collect();
    let list = List::new(items).block(left_block);
    frame.render_widget(list, horizontal_chunk_left);

    frame.render_widget(block, size);
}

fn print_thread(
    mut write_port: Box<dyn SerialPort>,
    diagnostic_channel: Receiver<String>,
    configuration_channel: Receiver<Vec<u8>>,
    packet_channel: Receiver<Vec<u8>>,
) {
    const INTERVAL: Duration = Duration::from_millis(30);
    const DEBOUNCE: Duration = Duration::from_millis(20);

    let mut diagnostic_messages: String = Default::default();
    let mut user_command: String = Default::default();
    let mut configuration_packets: Vec<Packet> = vec![];

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

    terminal
        .draw(|frame| {
            draw(
                frame,
                &diagnostic_messages,
                &user_command,
                &configuration_packets,
            )
        })
        .unwrap();
    let mut last_render = Instant::now();
    let mut debounce: Option<Instant> = None;

    loop {
        let timeout = debounce.map_or(INTERVAL, |start| DEBOUNCE.saturating_sub(start.elapsed()));
        if crossterm::event::poll(timeout).unwrap() {
            let refresh = match crossterm::event::read().unwrap() {
                Event::Key(key) => match key.code {
                    KeyCode::Esc => Refresh::Quit,
                    KeyCode::Enter => {
                        if !user_command.starts_with("/") {
                            user_command.push('\n');
                            // diagnostic_messages.push('\n');
                            let (data, size) = send_diagnostic(&user_command);
                            let _ = write_port.write(&data[..size]);
                            let _ = write_port.flush();
                        } else {
                            let mut request: CoapRequest<String> = CoapRequest::new();
                            request.set_method(Method::Get);
                            request.set_path(&user_command);
                            request.message.add_option(CoapOption::Block2, vec![0x05]);
                            let (data, size) = send_configuration(&request.message);
                            configuration_packets.push(request.message);
                            let _ = write_port.write(&data[..size]);
                            let _ = write_port.flush();
                        }
                        user_command.clear();
                        Refresh::Update
                    }
                    KeyCode::Backspace => {
                        user_command.pop();
                        Refresh::Update
                    }
                    KeyCode::Char(to_insert) => {
                        user_command.push(to_insert);
                        // diagnostic_messages.push(to_insert);
                        Refresh::Update
                    }
                    _ => Refresh::Skip,
                },
                Event::Resize(_, _) => Refresh::Update,
                Event::FocusGained | Event::FocusLost | Event::Paste(_) => Refresh::Skip,
                _ => Refresh::Skip,
            };
            match refresh {
                Refresh::Quit => return,
                Refresh::Skip => {}
                Refresh::Update => {
                    debounce.get_or_insert_with(Instant::now);
                }
            }
        }

        match diagnostic_channel.try_recv() {
            Ok(data) => {
                diagnostic_messages.push_str(&data);
                debounce.get_or_insert_with(Instant::now);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!();
            }
        }
        match configuration_channel.try_recv() {
            Ok(data) => {
                let response = Packet::from_bytes(&data).unwrap();
                configuration_packets.push(response);
                debounce.get_or_insert_with(Instant::now);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!();
            }
        }
        match packet_channel.try_recv() {
            Ok(_data) => {
                debounce.get_or_insert_with(Instant::now);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!();
            }
        }

        if debounce.map_or_else(
            || last_render.elapsed() > INTERVAL,
            |debounce| debounce.elapsed() > DEBOUNCE,
        ) {
            terminal
                .draw(|frame| {
                    draw(
                        frame,
                        &diagnostic_messages,
                        &user_command,
                        &configuration_packets,
                    )
                })
                .unwrap();
            last_render = Instant::now();
            debounce = None;
        }
    }
}

fn main() {
    let (diagnostic_tx, diagnostic_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (configuration_tx, configuration_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel();
    let (packet_tx, packet_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

    let conf_tx = configuration_tx.clone();

    let mut port = serialport::new("/dev/ttyACM1", 115200)
        .open()
        .expect("Error");
    let _ = port.set_timeout(Duration::from_secs(60));
    let read_port = port.try_clone().unwrap();
    let write_port = port.try_clone().unwrap();

    let _ =
        thread::spawn(move || read_thread(read_port, diagnostic_tx, configuration_tx, packet_tx));
    let ui_loop =
        thread::spawn(move || print_thread(write_port, diagnostic_rx, configuration_rx, packet_rx));

    // let (data, size) = send_diagnostic("help\n");
    // let _ = port.write(&data[..size]);
    // let _ = port.flush();

    let mut request: CoapRequest<String> = CoapRequest::new();

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
    ui_loop.join().unwrap();

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
