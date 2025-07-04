use std::cmp::max;
use std::env;
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
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;
use ratatui::Frame;
use tui_widgets::scrollview::ScrollView;

use crate::app::App;

impl App<'_> {
    fn render_header_footer(&self, frame: &mut Frame, header_area: Rect, footer_area: Rect) {
        let tab_titles = [
            "Overview (F1)",
            "Diagnostic (F2)",
            "Configuration (F3)",
            "Help (F5)",
        ];
        let title = if frame.area().width < 100 {
            "Jelly 🪼"
        } else {
            "Jelly 🪼: The friendly SLIPMUX for RIOT OS"
        };

        let title_area =
            Layout::horizontal([Constraint::Length(59), Constraint::Fill(1)]).split(header_area);

        frame.render_widget(
            Tabs::new(tab_titles)
                .highlight_style(Style::new().fg(Color::Black).bg(Color::White))
                .select(self.current_tab as usize)
                .padding("", "")
                .divider(" "),
            title_area[0],
        );
        frame.render_widget(
            Block::new()
                .borders(Borders::TOP)
                .title(title)
                .title_alignment(Alignment::Center),
            title_area[1],
        );

        let footer_title = match &self.write_port {
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
                .title(footer_title)
                .title_alignment(Alignment::Right),
            footer_area,
        );
    }

    fn render_help(&mut self, frame: &mut Frame, area: Rect) {
        let border_block = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Help")])
            .title_alignment(Alignment::Left);

        let text = r"Jelly 🪼: The friendly SLIPMUX for RIOT OS

Jelly is a tool that allows you to send commands to an attached device. The commands
are send via slipmux and can be either plain text or CoAP based. A device is typically
connected via a serial (e.g. /dev/ttyUSB0) but Jelly also supports connecting via a unix
socket, as long as the device speaks slipmux.

Hotkeys:
    Mousewheel: Scroll up and down.
    F1: Presents an overview, showing diagnostic messages next to the configuration messages.
    F2: Shows only the diagnostic messages.
    F3: Shows only the configuration messages.
    F5: This help :)
    ESC: Quit Jelly.
    RET: Send a command.
    TAB: Autocomplete.
    RIGHT: Autocomplete.

You can always type in a command into the `User Input` field. You do not need to select it.
When autocomplete is available, indicated by a light gray text in front of your cursor, 
press TAB or RIGHT to complete your input.

There are multiple classes of commands: 
- Raw diagnostic commands, these are written in all lowercase and are send as is to the device.
- CoAP endpoints, indicated by the leading `/`, send a CoAP GET request via a configuration
    message to its path. The response, if any, will be logged in the configuration view.
    Most endpoints are auto-discovered by Jelly but you can always send a GET request to any
    endpoint as long as it starts with `/`.
- Jelly commands, distinguishable by the leading uppercase letter, are commands that are 
    run mainly on your host and only communicate with the device via configuration messages.
    Every Jelly command offers usage information and help via the `--help` flag.
    These commands can issue multiple CoAP requests and may take time to complete. They render
    their own output into the diagnostic view and should look & feel close to the raw commands.
- If you type in something that is not recognized by Jelly, it will be send as a raw string
    via a diagnostic message to the device.

Saving output:
If you run a Jelly command, you can redirect the output into a file on your local filesystem.
`Saul > /tmp/saul.txt`
Some commands may allow exporting binary data. To export that use the `%>` redirect.
`Saul %> /tmp/saul.cbor`
If a command doesn't offer binary export, the `%>` will automatically downgrade to text export.
";
        let mut text = Text::from(text);
        let path = env::current_dir();
        match path {
            Ok(path) => {
                text.push_line(Line::from(format!(
                    "Your current working directory is: {:}",
                    path.display()
                )));
            }
            Err(_) => {
                text.push_line(Line::from("Your current working directory is unkown\n"));
            }
        }

        let content_width = border_block.inner(area).width;
        let messages_hight = u16::try_from(text.height()).unwrap_or(u16::MAX);

        let mut scroll_view = ScrollView::new(Size::new(
            // Make room for the scroll bar
            content_width - 1,
            messages_hight,
        ));

        if self.diagnostic_messages_scroll_follow {
            self.diagnostic_messages_scroll_state.scroll_to_bottom();
        }

        let paragraph = Paragraph::new(text);
        scroll_view.render_widget(
            paragraph,
            Rect::new(0, 0, content_width - 1, messages_hight),
        );

        frame.render_stateful_widget(
            scroll_view,
            border_block.inner(area),
            &mut self.diagnostic_messages_scroll_state,
        );

        frame.render_widget(border_block, area);
    }

    fn render_configuration_messages(&mut self, frame: &mut Frame, area: Rect) {
        let right_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Configuration Messages")])
            .title_alignment(Alignment::Left);

        let mut req_blocks = vec![];
        let mut constrains = vec![];
        let total_length: u16 = {
            let mut sum = 0;
            // temporay limitation to work around ratatui bug #1855
            let start = usize::try_from(max(
                i64::try_from(self.configuration_requests.len()).unwrap() - 10,
                0,
            ))
            .unwrap();
            for req in &self.configuration_requests[start..] {
                let block = Block::new()
                    .borders(Borders::TOP | Borders::BOTTOM)
                    .style(Style::new().gray())
                    // Realistically, there is exactly one line in a request; long term, we might
                    // want to use the rest too.
                    .title(
                        fmt_packet(&req.message)
                            .lines
                            .drain(..)
                            .next()
                            .unwrap_or_default(),
                    )
                    .title_alignment(Alignment::Left);
                if let Some(resp) = &req.response {
                    let text = fmt_packet(&resp.message);
                    let linecount = text.lines.len();
                    sum += linecount + 2;
                    constrains.push(Constraint::Min((linecount + 2).try_into().unwrap()));
                    req_blocks.push(Paragraph::new(text).block(block));
                } else {
                    req_blocks.push(Paragraph::new("Awaiting response").block(block));
                    sum += 3;
                    constrains.push(Constraint::Length(3));
                }
            }
            sum.try_into().unwrap_or(u16::MAX)
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
            .title_alignment(Alignment::Left);

        if self.user_input.is_empty() {
            let mut text = Text::from(
                Span::from("Type a command, for example: ").patch_style(Style::new().dark_gray()),
            );
            text.push_span(
                Span::from(self.known_commands.list_by_cmd().join(", "))
                    .patch_style(Style::new().dark_gray()),
            );
            let paragraph = Paragraph::new(text).block(right_block_down);
            frame.render_widget(paragraph, area);
            return;
        }
        let text: &str = &self.user_input;
        let mut text = Text::from(text);

        frame.set_cursor_position(Position::new(
            area.x + u16::try_from(self.user_input.len()).unwrap() + 1,
            area.y + 1,
        ));

        let (suggestion, cmds) = self
            .known_commands
            .longest_common_prefixed_by_cmd(&self.user_input);
        text.push_span(
            Span::from(suggestion.get(self.user_input.len()..).unwrap_or(""))
                .patch_style(Style::new().dark_gray()),
        );
        let command_options: String = match cmds.len() {
            0 => String::new(),
            1 => cmds[0].description.clone(),
            _ => cmds
                .iter()
                .map(|x| x.cmd.clone())
                .collect::<Vec<String>>()
                .join(" "),
        };

        text.push_span(
            Span::from(" | ".to_owned() + &command_options).patch_style(Style::new().dark_gray()),
        );
        let paragraph = Paragraph::new(text).block(right_block_down);
        frame.render_widget(paragraph, area);
    }

    fn render_diagnostic_messages(&mut self, frame: &mut Frame, area: Rect) {
        let left_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Diagnostic Messages")])
            .title_alignment(Alignment::Left);

        let content_width = left_block_up.inner(area).width;
        let messages_hight = u16::try_from(self.diagnostic_messages.height()).unwrap_or(u16::MAX);

        let mut scroll_view = ScrollView::new(Size::new(
            // Make room for the scroll bar
            content_width - 1,
            messages_hight,
        ));

        if self.diagnostic_messages_scroll_follow {
            self.diagnostic_messages_scroll_state.scroll_to_bottom();
        }

        scroll_view.render_widget(
            Paragraph::new(self.diagnostic_messages.clone()),
            Rect::new(0, 0, content_width - 1, messages_hight),
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
            .title(vec![Span::from("Board Info")])
            .title_alignment(Alignment::Left);

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

        self.render_header_footer(frame, header_area, footer_area);

        match self.current_tab {
            super::SelectedTab::Combined => {
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
                    .constraints([Constraint::Fill(1), Constraint::Length(3)].as_ref())
                    .split(main_chunk_left);

                let left_chunk_upper = left_chunks[0];
                let left_chunk_lower = left_chunks[1];

                self.render_configuration_messages(frame, right_chunk_upper);
                self.render_diagnostic_messages(frame, left_chunk_upper);
                self.render_configuration_overview(frame, right_chunk_lower);
                self.render_user_input(frame, left_chunk_lower);
            }
            super::SelectedTab::Diagnostic => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(3)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_diagnostic_messages(frame, chunk_upper);
                self.render_user_input(frame, chunk_lower);
            }
            super::SelectedTab::Configuration => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(3)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_configuration_messages(frame, chunk_upper);
                self.render_user_input(frame, chunk_lower);
            }
            super::SelectedTab::Help => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(3)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_help(frame, chunk_upper);
                self.render_user_input(frame, chunk_lower);
            }
        }
    }
}

fn fmt_packet(packet: &Packet) -> Text {
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
            let responsestyle = match rtype {
                // FIXME: Use classification instead
                coap_lite::ResponseType::Content => Style::new().green(),
                coap_lite::ResponseType::NotFound => Style::new().red(),
                _ => Style::new(),
            };

            let cf = packet.get_content_format();
            let payload_formatted = match (cf, &packet.payload) {
                (Some(ContentFormat::ApplicationLinkFormat), payload) => {
                    String::from_utf8_lossy(payload).replace(",<", ",\n  <")
                }
                (Some(ContentFormat::TextPlain), payload) => {
                    String::from_utf8_lossy(payload).to_string()
                }
                // this is a cheap-in-terms-of-dependencies hex formatting; `aa bb cc` would be
                // prettier than `[aa, bb, cc]`, but needs extra dependencies.
                (Some(ContentFormat::ApplicationCBOR), payload) => {
                    cbor_edn::StandaloneItem::from_cbor(payload).map_or_else(
                        |e| format!("Parsing error {e}, content {payload:02x?}"),
                        |c| c.serialize(),
                    )
                }
                (_, payload) => format!("{payload:02x?}"),
            };
            let slash_cf = cf.map(|c| format!("/{c:?}")).unwrap_or_default();
            let mut out = Text::default();
            let mut firstline = format!(
                " → Res({rtype:?}{slash_cf})[0x{:04x}]",
                u16::from_le_bytes(packet.get_token().try_into().unwrap_or([0xff, 0xff])),
            );
            let tail = if packet.payload.is_empty() {
                "  Empty Payload".to_owned()
            } else {
                _ = write!(firstline, ": {} bytes", packet.payload.len());
                format!("  {payload_formatted}")
            };
            out.lines.push(Line::styled(firstline, responsestyle));
            // FIXME: Is there no easier way to create an iterator of Line from a String?
            out.lines.extend(Text::from(tail).lines);
            return out;
        }
        MessageClass::Reserved(_) => _ = write!(out, "Reserved"),
    }
    Text::from(out)
}
