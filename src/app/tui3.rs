use std::fmt::Write;
use std::iter::zip;

use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Size;
use ratatui::prelude::Rect;
use ratatui::prelude::Stylize;
use ratatui::prelude::Widget;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use tui_scrollview::{ScrollView, ScrollViewState};

use coap_lite::CoapOption;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::Packet;

use crate::app::App;

impl App<'_> {
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

        let suggestion = self.suggest_command();
        let text: &str = &self.user_command;
        let mut text = Text::from(text);
        if let Some(suggestion) = suggestion {
            let cmd = &self.known_user_commands[suggestion];
            let typed_len = self.user_command.len();
            let suggestion_preview = cmd.get(typed_len..).unwrap();
            text.push_span(Span::from(suggestion_preview).patch_style(Style::new().dark_gray()));
        }
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

        let text = &self.diagnostic_messages;
        let height = left_block_up.inner(left_chunk_upper).height;
        let scroll = {
            if text.height() > height as usize {
                text.height() - height as usize
            } else {
                0
            }
        };
        let paragraph = Paragraph::new(self.diagnostic_messages.clone()).scroll((scroll as u16, 0));
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
