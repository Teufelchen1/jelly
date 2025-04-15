use std::fmt::Write;
use std::iter::zip;

use coap_lite::CoapOption;
use coap_lite::ContentFormat;
use coap_lite::MessageClass;
use coap_lite::Packet;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::Frame;
use tui_widgets::scrollview::ScrollView;

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
                let device_path = port;
                format!(
                    "✅ connected via {} with RIOT {}",
                    device_path, self.riot_version
                )
            }
            None => "❌ not connected, trying..".to_owned(),
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
                }
            }
            sum.try_into().unwrap()
        };

        let width = if right_block_up.inner(area).height < total_length {
            // Make room for the scroll bar
            right_block_up.inner(area).width - 1
        } else {
            right_block_up.inner(area).width
        };

        if self.configuration_scroll_follow {
            self.configuration_scroll_state.scroll_to_bottom();
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

        frame.render_stateful_widget(
            scroll_view,
            right_block_up.inner(area),
            &mut self.configuration_scroll_state,
        );
        frame.render_widget(right_block_up, area);
    }

    fn render_user_input(&self, frame: &mut Frame, area: Rect) {
        let right_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("User Input")])
            .title_alignment(Alignment::Left)
            .title_style(Style::new().white());

        let text: &str = &self.user_input;
        let mut text = Text::from(text);

        if let Some(suggestion) = self.suggest_command() {
            let cmd = &suggestion.cmd;
            let dscr = &suggestion.description;
            let typed_len = self.user_input.len();
            let suggestion_preview = cmd.get(typed_len..).unwrap();

            text.push_span(Span::from(suggestion_preview).patch_style(Style::new().dark_gray()));
            text.push_span(
                Span::from(" | ".to_owned() + dscr).patch_style(Style::new().dark_gray()),
            );
        }
        let paragraph = Paragraph::new(text).block(right_block_down);
        frame.render_widget(paragraph, area);
    }

    fn render_diagnostic_messages(&mut self, frame: &mut Frame, area: Rect) {
        //frame.render_widget(Clear, area);
        let left_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Diagnostic Messages")])
            .title_alignment(Alignment::Left)
            .title_style(Style::new().white());
        let content_width = left_block_up.inner(area).width;

        let mut scroll_view = ScrollView::new(Size::new(
            // Make room for the scroll bar
            content_width - 1,
            self.diagnostic_messages.height() as u16,
        ));

        if self.diagnostic_messages_scroll_follow {
            self.diagnostic_messages_scroll_state.scroll_to_bottom();
        }

        scroll_view.render_widget(
            Paragraph::new(self.diagnostic_messages.clone()),
            Rect::new(
                0,
                0,
                content_width - 1,
                self.diagnostic_messages.height() as u16,
            ),
        );

        frame.render_stateful_widget(
            scroll_view,
            left_block_up.inner(area),
            &mut self.diagnostic_messages_scroll_state,
        );
        frame.render_widget(left_block_up, area);
    }

    fn render_configuration_overview(&self, frame: &mut Frame, area: Rect) {
        let left_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Configuration")])
            .title_alignment(Alignment::Left)
            .title_style(Style::new().white());
        let text = format!(
            "Version: {}\nBoard: {}\n",
            self.riot_version, self.riot_board,
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
        .split(frame.area());
        let header_area = main_layout[0];
        let main_area = main_layout[1];
        let footer_area = main_layout[2];

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)].as_ref())
            .split(main_area);

        let main_chunk_left = main_chunks[0];
        let main_chunk_right = main_chunks[1];

        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Max(5)].as_ref())
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
            if let Some(option_list) = packet.get_option(CoapOption::UriPath) {
                for option in option_list {
                    _ = write!(out, "/{}", String::from_utf8_lossy(option));
                }
            } else {
                _ = write!(out, "/");
            }
            _ = write!(
                out,
                ")[0x{:04x}]",
                u16::from_le_bytes(packet.get_token().try_into().unwrap_or([0xff, 0xff]))
            );
        }
        MessageClass::Response(rtype) => {
            let cf = packet.get_content_format();
            let payload_formatted = match (cf, &packet.payload) {
                (Some(ContentFormat::ApplicationLinkFormat), payload) => {
                    // change me back | ContentFormat::TextPlain
                    String::from_utf8_lossy(payload).replace(',', "\n  ")
                }
                (Some(ContentFormat::TextPlain), payload) => {
                    String::from_utf8_lossy(payload).to_string()
                }
                // this is a cheap-in-terms-of-dependencies hex formatting; `aa bb cc` would be
                // prettier than `[aa, bb, cc]`, but needs extra dependencies.
                (_, payload) => format!("{payload:02x?}"),
            };
            let slash_cf = cf.map(|c| format!("/{c:?}")).unwrap_or_default();
            _ = write!(out, "");
            _ = write!(
                out,
                " → Res({rtype:?}{slash_cf})[0x{:04x}]",
                u16::from_le_bytes(packet.get_token().try_into().unwrap_or([0xff, 0xff])),
            );
            if packet.payload.is_empty() {
                _ = write!(out, "\n  Empty Payload");
            } else {
                _ = write!(
                    out,
                    ": {} bytes\n  {payload_formatted}",
                    packet.payload.len()
                );
            }
        }
        MessageClass::Reserved(_) => _ = write!(out, "Reserved"),
    }
    out
}
