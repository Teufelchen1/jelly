use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::{Packet, RequestType as Method};
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::prelude::Alignment;
use ratatui::prelude::Backend;
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
use serialport::SerialPort;

use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use std::time::Instant;

use crate::slipmux::send_configuration;
use crate::slipmux::send_diagnostic;

enum Refresh {
    /// Update the TUI
    Update,
    /// Skip the update of the TUI
    Skip,
    /// Quit the TUI and return to the shell
    Quit,
}

pub enum ElementInFocus {
    UserInput,
}

pub struct App {
    focus: ElementInFocus,
    user_command: String,
    diagnostic_messages: String,
    configuration_packets: Vec<Packet>,
    write_port: Box<dyn SerialPort>,
    diagnostic_channel: Receiver<String>,
    configuration_channel: Receiver<Vec<u8>>,
    packet_channel: Receiver<Vec<u8>>,
}

impl App {
    fn new(
        write_port: Box<dyn SerialPort>,
        diagnostic_channel: Receiver<String>,
        configuration_channel: Receiver<Vec<u8>>,
        packet_channel: Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            focus: ElementInFocus::UserInput,
            user_command: String::new(),
            diagnostic_messages: String::new(),
            configuration_packets: vec![],
            write_port,
            diagnostic_channel,
            configuration_channel,
            packet_channel,
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Refresh {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Refresh::Quit;
        }

        let update = match &self.focus {
            ElementInFocus::UserInput => match key.code {
                KeyCode::Esc => return Refresh::Quit,
                KeyCode::Enter => {
                    if !self.user_command.starts_with('/') {
                        self.user_command.push('\n');
                        let (data, size) = send_diagnostic(&self.user_command);
                        let _ = self.write_port.write(&data[..size]);
                    } else {
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Get);
                        request.set_path(&self.user_command);
                        request.message.add_option(CoapOption::Block2, vec![0x05]);
                        let (data, size) = send_configuration(&request.message);
                        self.configuration_packets.push(request.message);
                        let _ = self.write_port.write(&data[..size]);
                    }
                    let _ = self.write_port.flush();
                    self.user_command.clear();
                    true
                }
                KeyCode::Backspace => {
                    self.user_command.pop();
                    true
                }
                KeyCode::Char(to_insert) => {
                    self.user_command.push(to_insert);
                    // diagnostic_messages.push(to_insert);
                    true
                }
                _ => false,
            },
        };

        if update {
            Refresh::Update
        } else {
            Refresh::Skip
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
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

        let text: &str = &self.diagnostic_messages;
        let text = Text::from(text);
        let paragraph = Paragraph::new(text).block(right_block_up);
        frame.render_widget(paragraph, right_chunk_upper);

        let text: &str = &self.user_command;
        let text = Text::from(text);
        let paragraph = Paragraph::new(text).block(right_block_down);
        frame.render_widget(paragraph, right_chunk_lower);

        let left_block = Block::bordered()
            .title(vec![Span::from("Configuration Packets")])
            .title_alignment(Alignment::Left);
        let items: Vec<ListItem> = self
            .configuration_packets
            .iter()
            .map(|i| ListItem::new(fmt_packet(i)))
            .collect();
        let list = List::new(items).block(left_block);
        frame.render_widget(list, horizontal_chunk_left);

        frame.render_widget(block, size);
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

pub fn show(
    write_port: Box<dyn SerialPort>,
    diagnostic_channel: Receiver<String>,
    configuration_channel: Receiver<Vec<u8>>,
    packet_channel: Receiver<Vec<u8>>,
) {
    let app = App::new(
        write_port,
        diagnostic_channel,
        configuration_channel,
        packet_channel,
    );

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

    main_loop(app, terminal);

    reset_terminal();
}

fn main_loop<B>(mut app: App, mut terminal: Terminal<B>)
where
    B: Backend,
{
    const INTERVAL: Duration = Duration::from_millis(50);
    const DEBOUNCE: Duration = Duration::from_millis(20); // 50 FPS

    terminal.draw(|frame| app.draw(frame)).unwrap();

    let mut last_render = Instant::now();
    let mut debounce: Option<Instant> = None;

    loop {
        let timeout = debounce.map_or(INTERVAL, |start| DEBOUNCE.saturating_sub(start.elapsed()));
        if crossterm::event::poll(timeout).unwrap() {
            let refresh = match crossterm::event::read().unwrap() {
                Event::Key(key) => app.on_key(key),
                Event::Resize(_, _) => Refresh::Update,
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
        match app.diagnostic_channel.try_recv() {
            Ok(data) => {
                app.diagnostic_messages.push_str(&data);
                debounce.get_or_insert_with(Instant::now);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => panic!(),
        }
        match app.configuration_channel.try_recv() {
            Ok(data) => {
                let response = Packet::from_bytes(&data).unwrap();
                app.configuration_packets.push(response);
                debounce.get_or_insert_with(Instant::now);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => panic!(),
        }
        match app.packet_channel.try_recv() {
            Ok(_data) => {
                debounce.get_or_insert_with(Instant::now);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => panic!(),
        }
        if debounce.map_or_else(
            || last_render.elapsed() > INTERVAL,
            |debounce| debounce.elapsed() > DEBOUNCE,
        ) {
            terminal.draw(|frame| app.draw(frame)).unwrap();
            last_render = Instant::now();
            debounce = None;
        }
    }
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
