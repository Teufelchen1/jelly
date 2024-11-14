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

use crate::slipmux::send_diagnostic;
use serialport::SerialPort;

pub struct App {
    write_port: Box<dyn SerialPort>,
    diagnostic_messages: String,
    user_command: String,
    user_command_history: Vec<String>,
    user_command_cursor: usize,
}
impl App {
    pub fn new(write_port: Box<dyn SerialPort>) -> Self {
        Self {
            write_port,
            diagnostic_messages: String::new(),
            user_command: String::new(),
            user_command_history: vec![],
            user_command_cursor: 0,
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
                let (data, size) = send_diagnostic(&self.user_command);
                let _ = self.write_port.write(&data[..size]);
                if self.user_command != "\n" {
                    self.user_command_history.push(self.user_command.clone());
                    self.user_command_cursor = self.user_command_history.len();
                }
                self.user_command.clear();
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
        frame.render_widget(
            Block::new()
                .borders(Borders::TOP)
                .title(format!(
                    "✅ connected via /dev/ttyACM0 with RIOT {}",
                    0 //self.version
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
        // let mut req_blocks = vec![];
        //let mut constrains = vec![];
        // let total_length: u16 = {
        //     let mut sum = 0;
        //     for req in &self.configuration_requests {
        //         let option_list_ = req.message.get_option(CoapOption::UriPath).unwrap();
        //         let mut uri_path = String::new();
        //         for option in option_list_ {
        //             _ = write!(uri_path, "{}", String::from_utf8_lossy(option))
        //         }
        //         if uri_path.eq("configps") {
        //             let block = Block::new()
        //                 .borders(Borders::TOP | Borders::BOTTOM)
        //                 .title(vec![Span::from("Command: ps")])
        //                 .title_alignment(Alignment::Left);
        //             match &req.response {
        //                 Some(resp) => {
        //                     let text = fmt_ps(&resp.message);
        //                     let linecount = text.lines().count();
        //                     sum += linecount + 2;
        //                     constrains.push(Min((linecount + 2).try_into().unwrap()));
        //                     req_blocks.push(Paragraph::new(text).block(block));
        //                 }
        //                 None => {
        //                     req_blocks.push(Paragraph::new("Awaiting response").block(block));
        //                     sum += 3;
        //                     constrains.push(Min(3));
        //                 }
        //             };
        //         } else {
        //             let block = Block::new()
        //                 .borders(Borders::TOP | Borders::BOTTOM)
        //                 .title(vec![Span::from(fmt_packet(&req.message))])
        //                 .title_alignment(Alignment::Left);
        //             match &req.response {
        //                 Some(resp) => {
        //                     let text = fmt_packet(&resp.message);
        //                     let linecount = text.lines().count();
        //                     sum += linecount + 2;
        //                     constrains.push(Min((linecount + 2).try_into().unwrap()));
        //                     req_blocks.push(Paragraph::new(text).block(block));
        //                 }
        //                 None => {
        //                     req_blocks.push(Paragraph::new("Awaiting response").block(block));
        //                     sum += 3;
        //                     constrains.push(Min(3));
        //                 }
        //             };
        //         }
        //     }
        //     sum.try_into().unwrap()
        // };

        let total_length = 0;

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
        //let areas: Vec<Rect> = Layout::vertical(constrains).split(area).to_vec();
        // for (a, req_b) in zip(areas, req_blocks) {
        //     req_b.render(a, buf);
        // }
        // for _request in &self.configuration_requests {}
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
            "Hello" //"Version: {}\nBoard: {}\n{}",
                    //self.version, self.board, self.ip
        );
        let text = Text::from(text);
        let paragraph = Paragraph::new(text);
        let paragraph_block = paragraph.block(left_block_down);
        frame.render_widget(paragraph_block, left_chunk_lower);
    }
}
