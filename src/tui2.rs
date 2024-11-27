use std::fmt::Write;
use std::iter::zip;
use std::time::Duration;
use std::{thread, time};

use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Size;
use ratatui::prelude::Rect;
use ratatui::prelude::StatefulWidget;
use ratatui::prelude::Widget;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use tui_scrollview::{ScrollView, ScrollViewState};

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::CoapResponse;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::Packet;
use coap_lite::RequestType as Method;

use crate::slipmux::{send_configuration, send_diagnostic};
use serialport::SerialPort;

pub struct App {
    write_port: Option<Box<dyn SerialPort>>,
    configuration_requests: Vec<CoapRequest<String>>,
    configuration_packets: Vec<Packet>,
    diagnostic_messages: String,
    user_command: String,
    user_command_history: Vec<String>,
    user_command_cursor: usize,
    token_count: u16,
    riot_board: String,
    riot_version: String,
}
impl App {
    pub fn new() -> Self {
        Self {
            write_port: None,
            configuration_requests: vec![],
            configuration_packets: vec![],
            diagnostic_messages: String::new(),
            user_command: String::new(),
            user_command_history: vec![],
            user_command_cursor: 0,
            token_count: 0,
            riot_board: "Unkown".to_string(),
            riot_version: "Unkown".to_string(),
        }
    }

    fn get_new_token(&mut self) -> Vec<u8> {
        self.token_count += 1;
        self.token_count.to_le_bytes().to_vec()
    }

    fn build_request(&mut self, path: &str) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(path);
        request.message.set_token(self.get_new_token());
        request.message.add_option(CoapOption::Block2, vec![0x05]);
        request
    }

    fn send_request(&mut self, msg: &Packet) {
        let (data, size) = send_configuration(msg);
        if let Some(ref mut port) = &mut self.write_port {
            let _ = port.write_all(&data[..size]);
            let _ = port.flush();
        }
    }

    pub fn connect(&mut self, write_port: Box<dyn SerialPort>) {
        self.write_port = Some(write_port);

        let request: CoapRequest<String> = self.build_request("/riot/board");
        self.send_request(&request.message);
        self.configuration_requests.push(request);

        // TODO: Fix me
        thread::sleep(time::Duration::from_millis(10));

        let request: CoapRequest<String> = self.build_request("/riot/ver");
        self.send_request(&request.message);
        self.configuration_requests.push(request);

        // TODO: Fix me
        // thread::sleep(time::Duration::from_millis(20));

        // let request: CoapRequest<String> = self.build_request("/.well-known/core");
        // self.send_request(&request.message);
        // self.configuration_requests.push(request);
    }

    pub fn disconnect(&mut self) {
        self.write_port = None;
    }

    pub fn on_configuration_msg(&mut self, data: Vec<u8>) {
        let response = Packet::from_bytes(&data).unwrap();
        let token = response.get_token();
        let mut was_response = false;
        for request in &mut self.configuration_requests {
            if request.message.get_token() == token {
                request.response = Some(CoapResponse {
                    message: response.clone(),
                });

                let option_list_ = request.message.get_option(CoapOption::UriPath).unwrap();
                let mut uri_path = String::new();
                for option in option_list_ {
                    _ = write!(uri_path, "/{}", String::from_utf8_lossy(option))
                }
                match uri_path.as_str() {
                    "/riot/board" => {
                        self.riot_board = String::from_utf8_lossy(&response.payload).to_string()
                    }
                    "/riot/ver" => {
                        self.riot_version = String::from_utf8_lossy(&response.payload).to_string()
                    }
                    _ => (),
                }
                was_response = true;
            }
        }

        // This should never happen, as it means that the riot node
        // proactively send a configuration message
        if !was_response {
            self.configuration_packets.push(response);
        }
    }

    pub fn on_diagnostic_msg(&mut self, msg: String) {
        self.diagnostic_messages.push_str(&msg);
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }

        match key.code {
            KeyCode::Esc => return false,
            KeyCode::Enter => {
                if !self.user_command.ends_with('\n') {
                    self.user_command.push('\n');
                }
                if let Some(ref mut port) = &mut self.write_port {
                    let (data, size) = send_diagnostic(&self.user_command);
                    let _ = port.write(&data[..size]);
                    if self.user_command != "\n" {
                        self.user_command_history.push(self.user_command.clone());
                        self.user_command_cursor = self.user_command_history.len();
                    }
                    self.user_command.clear();
                }
            }
            KeyCode::Backspace => {
                self.user_command.pop();
            }
            KeyCode::Up => {
                if self.user_command_cursor > 0 {
                    self.user_command.clear();
                    self.user_command_cursor -= 1;
                    self.user_command = self.user_command_history[self.user_command_cursor].clone();
                }
            }
            KeyCode::Down => {
                if self.user_command_cursor < self.user_command_history.len() {
                    self.user_command.clear();
                    self.user_command_cursor += 1;
                    if self.user_command_cursor == self.user_command_history.len() {
                        self.user_command.clear();
                    } else {
                        self.user_command =
                            self.user_command_history[self.user_command_cursor].clone();
                    }
                }
            }
            KeyCode::Char(to_insert) => {
                self.user_command.push(to_insert);
            }
            _ => return false,
        };
        true
    }

    pub fn draw(&mut self, frame: &mut Frame) {
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
        let title = match &self.write_port {
            Some(port) => {
                let name = port.name().unwrap_or("<unkown>".to_string());
                format!(
                    "✅ connected via {} with RIOT {}",
                    name,
                    0 //self.version
                )
            }
            None => format!("❌ not connected, trying.. /dev/ttyACM0"),
        };
        frame.render_widget(
            Block::new()
                .borders(Borders::TOP)
                .title(title)
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
                let block = Block::new()
                    .borders(Borders::TOP | Borders::BOTTOM)
                    .title(vec![Span::from(fmt_packet(&req.message))])
                    .title_alignment(Alignment::Left);
                match &req.response {
                    Some(resp) => {
                        let text = fmt_packet(&resp.message);
                        let linecount = text.lines().count();
                        sum += linecount + 2;
                        constrains.push(Constraint::Min((linecount + 2).try_into().unwrap()));
                        req_blocks.push(Paragraph::new(text).block(block));
                    }
                    None => {
                        req_blocks.push(Paragraph::new("Awaiting response").block(block));
                        sum += 3;
                        constrains.push(Constraint::Min(3));
                    }
                };
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

        let text = format!(
            "Version: {}\nBoard: {}\n",
            self.riot_version, self.riot_board,
        );
        let text = Text::from(text);
        let paragraph = Paragraph::new(text);
        let paragraph_block = paragraph.block(left_block_down);
        frame.render_widget(paragraph_block, left_chunk_lower);
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
