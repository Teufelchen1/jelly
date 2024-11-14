use crate::tui::Constraint::Fill;
use crate::tui::Constraint::Length;
use crate::tui::Constraint::Max;
use crate::tui::Constraint::Min;
use coap_lite::CoapResponse;
use core::iter::zip;
use ratatui::prelude::Rect;
use ratatui::prelude::StatefulWidget;
use ratatui::prelude::Widget;
use ratatui::widgets::Borders;
use std::borrow::Cow;
use std::fmt::Write;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use std::time::Instant;
use tui_scrollview::{ScrollView, ScrollViewState};

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::Packet;
use coap_lite::RequestType as Method;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Size;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui::Terminal;
use serialport::SerialPort;

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
    ip: String,
    version: String,
    board: String,
    token_count: u16,
    user_commands: Vec<String>,
    user_command: String,
    user_command_cursor: usize,
    autocomplete: Vec<String>,
    diagnostic_messages: String,
    configuration_requests: Vec<CoapRequest<String>>,
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
            ip: String::new(),
            version: String::new(),
            board: String::new(),
            token_count: 0,
            user_commands: vec![],
            user_command: String::new(),
            user_command_cursor: 0,
            autocomplete: vec![
                "help".to_string(),
                "/.well-known/".to_string(),
                "/.well-known/core".to_string(),
                "/.well-known/ifconfig".to_string(),
                "/sha256".to_string(),
                "/riot/".to_string(),
                "/echo/".to_string(),
                "/shell/".to_string(),
                "/shell/version".to_string(),
                "/shell/nib".to_string(),
                "/shell/reboot".to_string(),
                "/shell/saul".to_string(),
                "/shell/ps_regular".to_string(),
                "/shell/pm".to_string(),
                "/shell/txtsnd".to_string(),
                "/shell/ifconfig".to_string(),
                "/config/ps".to_string(),
                "ifconfig".to_string(),
                "nib".to_string(),
                "pm".to_string(),
                "ps_regular".to_string(),
                "reboot".to_string(),
                "saul".to_string(),
                "txtsnd".to_string(),
                "version".to_string(),
            ],
            diagnostic_messages: String::new(),
            configuration_requests: vec![],
            configuration_packets: vec![],
            write_port,
            diagnostic_channel,
            configuration_channel,
            packet_channel,
        }
    }

    fn get_new_token(&mut self) -> Vec<u8> {
        self.token_count += 1;
        self.token_count.to_le_bytes().to_vec()
    }

    fn poll_ifconfig(&mut self) {
        {
            let mut request: CoapRequest<String> = CoapRequest::new();
            request.set_method(Method::Get);
            request.set_path("/riot/ver");
            request.message.add_option(CoapOption::Block2, vec![0x05]);
            let (data, size) = send_configuration(&request.message);
            let _ = self.write_port.write(&data[..size]);

            let mut version = String::new();
            match self.configuration_channel.recv() {
                Ok(data) => {
                    let response = Packet::from_bytes(&data);
                    if response.is_ok() {
                        _ = write!(
                            version,
                            "{}",
                            String::from_utf8_lossy(&response.unwrap().payload)
                        );
                    } else {
                        _ = write!(
                            version,
                            "{}",
                            String::from_utf8_lossy(b"Failed to parse /riot/ver packet")
                        );
                    }
                }
                Err(_) => panic!(),
            }
            let version = version
                .split_once('(')
                .unwrap()
                .1
                .split_once(')')
                .unwrap()
                .0;
            _ = write!(self.version, "{}", version);
        }
        {
            let mut request: CoapRequest<String> = CoapRequest::new();
            request.set_method(Method::Get);
            request.set_path("/riot/board");
            request.message.add_option(CoapOption::Block2, vec![0x05]);
            let (data, size) = send_configuration(&request.message);
            let _ = self.write_port.write(&data[..size]);

            match self.configuration_channel.recv() {
                Ok(data) => {
                    let response = Packet::from_bytes(&data);
                    if response.is_ok() {
                        _ = write!(
                            self.board,
                            "{:}",
                            String::from_utf8_lossy(&response.unwrap().payload)
                        );
                    } else {
                        _ = write!(self.board, "Failed to parse /riot/board packet");
                    }
                }
                Err(_) => panic!(),
            }
        }
        {
            let mut request: CoapRequest<String> = CoapRequest::new();
            request.set_method(Method::Get);
            request.set_path("/.well-known/ifconfig");
            request.message.add_option(CoapOption::Block2, vec![0x05]);
            let (data, size) = send_configuration(&request.message);
            let _ = self.write_port.write(&data[..size]);

            match self.configuration_channel.recv() {
                Ok(data) => {
                    let response = Packet::from_bytes(&data);
                    if response.is_ok() {
                        _ = write!(
                            self.ip,
                            "{:}",
                            String::from_utf8_lossy(&response.unwrap().payload)
                        );
                    } else {
                        _ = write!(self.ip, "Failed to parse ifconfig packet");
                    }
                }
                Err(_) => panic!(),
            }
        }
    }

    fn suggest_cmd(&self, cmd: &String) -> Option<String> {
        for known_cmd in &self.autocomplete {
            if known_cmd.starts_with(cmd) {
                return Some(known_cmd.clone());
            };
        }
        None
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
                        if !self.user_command.ends_with('\n') {
                            self.user_command.push('\n');
                        }
                        let (data, size) = send_diagnostic(&self.user_command);
                        let _ = self.write_port.write(&data[..size]);
                    } else {
                        let mut request: CoapRequest<String> = CoapRequest::new();
                        request.set_method(Method::Get);
                        request.set_path(&self.user_command);
                        request.message.set_token(self.get_new_token());
                        request.message.add_option(CoapOption::Block2, vec![0x05]);
                        let (data, size) = send_configuration(&request.message);
                        self.configuration_packets.push(request.message.clone());
                        let _ = self.write_port.write(&data[..size]);
                        self.configuration_requests.push(request);
                    }
                    let _ = self.write_port.flush();
                    if self.user_command != "\n" {
                        self.user_commands.push(self.user_command.clone());
                        self.user_command_cursor = self.user_commands.len();
                    }
                    self.user_command.clear();
                    true
                }
                KeyCode::Backspace => {
                    self.user_command.pop();
                    true
                }
                KeyCode::Up => {
                    if self.user_command_cursor > 0 {
                        self.user_command.clear();
                        self.user_command_cursor -= 1;
                        self.user_command = self.user_commands[self.user_command_cursor].clone();
                    }
                    true
                }
                KeyCode::Down => {
                    if self.user_command_cursor < self.user_commands.len() {
                        self.user_command.clear();
                        self.user_command_cursor += 1;
                        if self.user_command_cursor == self.user_commands.len() {
                            self.user_command.clear();
                        } else {
                            self.user_command =
                                self.user_commands[self.user_command_cursor].clone();
                        }
                    }
                    true
                }
                KeyCode::Tab => {
                    match self.suggest_cmd(&self.user_command) {
                        Some(cmd) => self.user_command = cmd,
                        None => {}
                    }
                    true
                }
                KeyCode::Char(to_insert) => {
                    self.user_command.push(to_insert);
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
        let main_layout = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ],
        )
        .split(frame.size());
        frame.render_widget(
            Block::new()
                .borders(Borders::TOP)
                .title("Jelly 🪼: Friendly SLIPMUX for RIOT OS")
                .title_alignment(Alignment::Center),
            main_layout[0],
        );
        frame.render_widget(
            Block::new()
                .borders(Borders::TOP)
                .title(format!(
                    "✅ connected via /dev/ttyACM0 with RIOT {}",
                    self.version
                ))
                .title_alignment(Alignment::Right),
            main_layout[2],
        );

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(main_layout[1]);

        let horizontal_chunk_left = horizontal_chunks[0];
        let horizontal_chunk_right = horizontal_chunks[1];

        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
            .split(horizontal_chunk_right);

        let right_chunk_upper = right_chunks[0];
        let right_chunk_lower = right_chunks[1];

        let right_block_up = Block::bordered()
            .title(vec![Span::from("Configuration Messages")])
            .title_alignment(Alignment::Left);

        let right_block_down = Block::bordered()
            .title(vec![Span::from("User Input")])
            .title_alignment(Alignment::Left);

        let text: &str = &self.user_command;
        let text = Text::from(text);
        let paragraph = Paragraph::new(text).block(right_block_down);
        frame.render_widget(paragraph, right_chunk_lower);

        let mut state = ScrollViewState::default();
        let mut req_blocks = vec![];
        let mut constrains = vec![];
        let total_length: u16 = {
            let mut sum = 0;
            for req in &self.configuration_requests {
                let option_list_ = req.message.get_option(CoapOption::UriPath).unwrap();
                let mut uri_path = String::new();
                for option in option_list_ {
                    _ = write!(uri_path, "{}", String::from_utf8_lossy(option))
                }
                if uri_path.eq("configps") {
                    let block = Block::new()
                        .borders(Borders::TOP | Borders::BOTTOM)
                        .title(vec![Span::from("Command: ps")])
                        .title_alignment(Alignment::Left);
                    match &req.response {
                        Some(resp) => {
                            let text = fmt_ps(&resp.message);
                            let linecount = text.lines().count();
                            sum += linecount + 2;
                            constrains.push(Min((linecount + 2).try_into().unwrap()));
                            req_blocks.push(Paragraph::new(text).block(block));
                        }
                        None => {
                            req_blocks.push(Paragraph::new("Awaiting response").block(block));
                            sum += 3;
                            constrains.push(Min(3));
                        }
                    };
                } else {
                    let block = Block::new()
                        .borders(Borders::TOP | Borders::BOTTOM)
                        .title(vec![Span::from(fmt_packet(&req.message))])
                        .title_alignment(Alignment::Left);
                    match &req.response {
                        Some(resp) => {
                            let text = fmt_packet(&resp.message);
                            let linecount = text.lines().count();
                            sum += linecount + 2;
                            constrains.push(Min((linecount + 2).try_into().unwrap()));
                            req_blocks.push(Paragraph::new(text).block(block));
                        }
                        None => {
                            req_blocks.push(Paragraph::new("Awaiting response").block(block));
                            sum += 3;
                            constrains.push(Min(3));
                        }
                    };
                }
            }
            sum.try_into().unwrap()
        };

        let width = if right_block_up.inner(right_chunk_upper).height < total_length {
            right_block_up.inner(right_chunk_upper).width - 1
        } else {
            right_block_up.inner(right_chunk_upper).width
        };

        if right_block_up.inner(right_chunk_upper).height < total_length {
            let diff = total_length - right_block_up.inner(right_chunk_upper).height;
            for _ in 0..diff {
                state.scroll_down();
            }
        }

        let mut scroll_view = ScrollView::new(Size::new(width, total_length));
        let buf = scroll_view.buf_mut();
        let area = buf.area;
        let areas: Vec<Rect> = Layout::vertical(constrains).split(area).to_vec();
        for (a, req_b) in zip(areas, req_blocks) {
            req_b.render(a, buf);
        }
        for _request in &self.configuration_requests {}
        frame.render_stateful_widget(
            scroll_view,
            right_block_up.inner(right_chunk_upper),
            &mut state,
        );
        frame.render_widget(right_block_up, right_chunk_upper);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
            .split(horizontal_chunk_left);

        let left_chunk_upper = left_chunks[0];
        let left_chunk_lower = left_chunks[1];

        let left_block_up = Block::bordered()
            .title(vec![Span::from("Diagnostic Messages")])
            .title_alignment(Alignment::Left);

        let left_block_down = Block::bordered()
            .title(vec![Span::from("Configuration")])
            .title_alignment(Alignment::Left);

        let text: &str = &self.diagnostic_messages;
        let text = Text::from(text);
        let height = left_block_up.inner(left_chunk_upper).height;
        let scroll = {
            if text.height() > height as usize {
                text.height() - height as usize
            } else {
                0
            }
        };
        let paragraph = Paragraph::new(text).scroll((scroll as u16, 0));
        let paragraph_block = paragraph.block(left_block_up);
        frame.render_widget(paragraph_block, left_chunk_upper);

        //let text: &str = &self.ip;
        let text = format!(
            "Version: {}\nBoard: {}\n{}",
            self.version, self.board, self.ip
        );
        let text = Text::from(text);
        let paragraph = Paragraph::new(text);
        let paragraph_block = paragraph.block(left_block_down);
        frame.render_widget(paragraph_block, left_chunk_lower);
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
    let mut app = App::new(
        write_port,
        diagnostic_channel,
        configuration_channel,
        packet_channel,
    );
    //app.poll_ifconfig();

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
                let token = response.get_token();
                for request in &mut app.configuration_requests {
                    if request.message.get_token() == token {
                        request.response = Some(CoapResponse {
                            message: response.clone(),
                        });
                    }
                }
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
    // When writing to a String `write!` will never fail.
    // Therefore the Result is ignored with `_ = write!()`.
    let mut out = String::new();
    match packet.header.code {
        MessageClass::Empty => _ = write!(out, "Empty"),
        MessageClass::Request(rtype) => {
            _ = write!(out, " ← Req({rtype:?} ");
            let option_list = packet.get_option(CoapOption::UriPath).unwrap();
            for option in option_list {
                _ = write!(out, "/{}", String::from_utf8_lossy(option));
            }
            _ = write!(
                out,
                ")[0x{:04x}]",
                u16::from_le_bytes(packet.get_token().try_into().unwrap_or([0xff, 0xff]))
            );
        }
        MessageClass::Response(rtype) => {
            _ = write!(out, " → Res({rtype:?}");
            if let Some(cf) = packet.get_content_format() {
                let payload = match cf {
                    ContentFormat::ApplicationLinkFormat => {
                        // change me back | ContentFormat::TextPlain
                        String::from_utf8_lossy(&packet.payload).replace(',', "\n  ")
                    }
                    ContentFormat::TextPlain => {
                        String::from_utf8_lossy(&packet.payload).to_string()
                    }
                    _ => todo!(),
                };
                _ = write!(
                    out,
                    "/{cf:?})[0x{:04x}] {:} bytes\n  {payload}",
                    u16::from_le_bytes(packet.get_token().try_into().unwrap_or([0xff, 0xff])),
                    payload.len()
                );
            } else {
                _ = write!(
                    out,
                    ")[0x{:04x}]\n  Empty Payload",
                    u16::from_le_bytes(packet.get_token().try_into().unwrap_or([0xff, 0xff]))
                );
            }
        }
        MessageClass::Reserved(_) => _ = write!(out, "Reserved"),
    }
    out
}

fn fmt_ps(packet: &Packet) -> String {
    // When writing to a String `write!` will never fail.
    // Therefore the Result is ignored with `_ = write!()`.
    let mut out = String::new();
    match packet.header.code {
        MessageClass::Empty => _ = write!(out, "Empty"),
        MessageClass::Request(_rtype) => {
            _ = write!(out, "Request");
        }
        MessageClass::Response(_rtype) => {
            if let Some(cf) = packet.get_content_format() {
                let _payload = match cf {
                    ContentFormat::TextPlain => {
                        //_ = write!(out, "Total payload size: {}\n", packet.payload.len());
                        _ = write!(
                            out,
                            "{:<20}|{:<5}|{:<5}|{:<5}|{:<10}|{:<10}|\n",
                            "name", "stack", "used", "free", "start", "SP"
                        );
                        let mut last_zero = 0;
                        while let Some(mut next_zero) =
                            packet.payload[last_zero..].iter().position(|&x| x == 0)
                        {
                            next_zero = last_zero + next_zero;
                            let name =
                                String::from_utf8_lossy(&packet.payload[last_zero..next_zero]);
                            next_zero += 1;

                            let stack_size = u32::from_le_bytes(
                                packet.payload[next_zero..(next_zero + 4)]
                                    .try_into()
                                    .unwrap(),
                            );
                            next_zero += 4;
                            let stack_size_used = u32::from_le_bytes(
                                packet.payload[next_zero..(next_zero + 4)]
                                    .try_into()
                                    .unwrap(),
                            );
                            next_zero += 4;
                            let stack_start = u32::from_le_bytes(
                                packet.payload[next_zero..(next_zero + 4)]
                                    .try_into()
                                    .unwrap(),
                            );
                            next_zero += 4;
                            let stack_pointer = stack_start + stack_size + 52 - stack_size_used;
                            let stack_free = stack_size - stack_size_used;
                            _ = write!(
                                out,
                                "{name:<20}|{stack_size:<5}|{stack_size_used:<5}|{stack_free:<5}|{stack_start:#010x}|{stack_pointer:#010x}|\n"
                            );
                            //next_zero += 4;
                            last_zero = next_zero;
                        }
                    }
                    _ => todo!(),
                };
            } else {
                _ = write!(
                    out,
                    ")[0x{:04x}]\n  Empty Payload",
                    u16::from_le_bytes(packet.get_token().try_into().unwrap_or([0xff, 0xff]))
                );
            }
        }
        MessageClass::Reserved(_) => _ = write!(out, "Reserved"),
    }
    out
}
