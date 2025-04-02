use std::fmt::Write;
use std::iter::zip;

use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Position;
use ratatui::layout::Size;
use ratatui::prelude::Rect;
use ratatui::prelude::Stylize;
use ratatui::prelude::Widget;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::{Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use tui_scrollview::{ScrollView, ScrollViewState};

use coap_lite::CoapOption;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::Packet;

use crate::app::App;

impl App<'_> {
    fn render_header_footer(&self, frame: &mut Frame, header_area: Rect, footer_area: Rect) {
        frame.render_widget(
            Block::new()
                .borders(Borders::TOP)
                .title("Jelly 🪼: The friendly SLIPMUX for RIOT OS")
                .title_alignment(Alignment::Center),
            header_area,
        );
        let title = match &self.write_port {
            Some(port) => {
                let device_path = port.name();
                format!(
                    "✅ connected via {} with RIOT {}",
                    device_path, self.riot_version
                )
            }
            None => format!("❌ not connected, trying.."),
        };
        frame.render_widget(
            Block::new()
                .borders(Borders::TOP)
                .title(title)
                .title_alignment(Alignment::Right),
            footer_area,
        );
    }

    fn render_configuration_messages(&mut self, frame: &mut Frame, area: Rect) {
        let right_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Configuration Messages")])
            .title_alignment(Alignment::Left)
            .title_style(Style::new().white());

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
                    .style(Style::new().gray())
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

        let width = if right_block_up.inner(area).height < total_length {
            // Make room for the scroll bar
            right_block_up.inner(area).width - 1
        } else {
            right_block_up.inner(area).width
        };

        if self.configuration_scroll_position_follow {
            self.configuration_scroll_position.scroll_to_bottom();
        }

        let mut scroll_view = ScrollView::new(Size::new(width, total_length));
        let buf = scroll_view.buf_mut();
        let scroll_view_area = buf.area;
        let areas: Vec<Rect> = Layout::vertical(constrains)
            .split(scroll_view_area)
            .to_vec();
        for (a, req_b) in zip(areas, req_blocks) {
            req_b.render(a, buf);
        }
        for _request in &self.configuration_requests {}
        frame.render_stateful_widget(
            scroll_view,
            right_block_up.inner(area),
            &mut self.configuration_scroll_position,
        );
        frame.render_widget(right_block_up, area);
    }

    fn render_user_input(&self, frame: &mut Frame, area: Rect) {
        let right_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("User Input")])
            .title_alignment(Alignment::Left)
            .title_style(Style::new().white());

        let suggestion = self.suggest_command();
        let text: &str = &self.user_command;
        let mut text = Text::from(text);
        if let Some(suggestion) = suggestion {
            let cmd = &self.known_user_commands[suggestion].cmd;
            let dscr = &self.known_user_commands[suggestion].description;
            let typed_len = self.user_command.len();
            let suggestion_preview = cmd.get(typed_len..).unwrap();
            text.push_span(Span::from(suggestion_preview).patch_style(Style::new().dark_gray()));
            text.push_span(
                Span::from(" | ".to_owned() + dscr).patch_style(Style::new().dark_gray()),
            );
        }
        let paragraph = Paragraph::new(text).block(right_block_down);
        frame.render_widget(paragraph, area);
    }

    fn render_diagnostic_messages(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let left_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Diagnostic Messages")])
            .title_alignment(Alignment::Left)
            .title_style(Style::new().white());
        let text = &self.diagnostic_messages;
        let text_height = text.height();
        let height = left_block_up.inner(area).height;
        // let scroll = {
        //     if text_height > height as usize {
        //         text_height - height as usize
        //     } else {
        //         0
        //     }
        // };
        let paragraph = Paragraph::new(text.clone())
            .scroll((self.diagnostic_messages_scroll_position as u16, 0));
        let paragraph_block = paragraph.block(left_block_up);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(text_height).position(self.diagnostic_messages_scroll_position);
        frame.render_widget(paragraph_block, area);
        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                // using an inner vertical margin of 1 unit makes the scrollbar inside the block
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }

    fn render_configuration_overview(&self, frame: &mut Frame, area: Rect) {
        let left_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Configuration")])
            .title_alignment(Alignment::Left)
            .title_style(Style::new().white());
        let text = format!(
            "Version: {}\nBoard: {}\n{},{}\n",
            self.riot_version,
            self.riot_board,
            self.configuration_scroll_position.offset().x,
            self.configuration_scroll_position.offset().y
        );
        let text = Text::from(text);
        let paragraph = Paragraph::new(text);
        let paragraph_block = paragraph.block(left_block_down);

        frame.render_widget(paragraph_block, area);
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
        let header_area = main_layout[0];
        let main_area = main_layout[1];
        let footer_area = main_layout[2];

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(main_area);

        let main_chunk_left = main_chunks[0];
        let main_chunk_right = main_chunks[1];

        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
            .split(main_chunk_right);

        let right_chunk_upper = right_chunks[0];
        let right_chunk_lower = right_chunks[1];

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Max(3)].as_ref())
            .split(main_chunk_left);

        let left_chunk_upper = left_chunks[0];
        let left_chunk_lower = left_chunks[1];

        self.render_header_footer(frame, header_area, footer_area);

        self.render_configuration_messages(frame, right_chunk_upper);

        self.render_user_input(frame, left_chunk_lower);

        self.render_diagnostic_messages(frame, left_chunk_upper);

        self.render_configuration_overview(frame, right_chunk_lower);
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
