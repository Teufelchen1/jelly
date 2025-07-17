use std::cmp::max;
use std::cmp::min;
use std::env;
use std::iter::zip;

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

impl App {
    fn render_header_footer(&self, frame: &mut Frame, header_area: Rect, footer_area: Rect) {
        let tab_titles = [
            "Overview (F1)",
            "Diagnostic (F2)",
            "Configuration (F3)",
            "Commands (F4)",
            "Help (F5)",
        ];
        let title = if frame.area().width < 100 {
            "Jelly 🪼"
        } else {
            "Jelly 🪼: The friendly SLIPMUX for RIOT OS"
        };

        let title_area =
            Layout::horizontal([Constraint::Length(72), Constraint::Fill(1)]).split(header_area);

        frame.render_widget(
            Tabs::new(tab_titles)
                .highlight_style(Style::new().fg(Color::Black).bg(Color::White))
                .select(self.ui_state.current_tab as usize)
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

        let footer_title = self.ui_state.get_connection();
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

        let paragraph = Paragraph::new(text);
        scroll_view.render_widget(
            paragraph,
            Rect::new(0, 0, content_width - 1, messages_hight),
        );

        frame.render_stateful_widget(
            scroll_view,
            border_block.inner(area),
            &mut self.ui_state.diagnostic_messages_scroll_state,
        );

        frame.render_widget(border_block, area);
    }

    fn render_commands(&mut self, frame: &mut Frame, area: Rect) {
        let right_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Commands")])
            .title_alignment(Alignment::Left);

        let mut req_blocks = vec![];
        let mut constrains = vec![];
        let total_length: u16 = {
            let mut sum = 0;
            // temporay limitation to work around ratatui bug #1855
            let start =
                usize::try_from(max(i64::try_from(self.job_log.jobs.len()).unwrap() - 10, 0))
                    .unwrap();
            for job in &mut self.job_log.jobs[start..] {
                let (size, para) = job.paragraph();
                req_blocks.push(para);
                sum += size;
                constrains.push(Constraint::Length(size.try_into().unwrap()));
            }
            sum.try_into().unwrap_or(u16::MAX)
        };

        let width = if right_block_up.inner(area).height < total_length {
            // Make room for the scroll bar
            right_block_up.inner(area).width - 1
        } else {
            right_block_up.inner(area).width
        };

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
            &mut self.ui_state.configuration_scroll_state,
        );
        frame.render_widget(right_block_up, area);
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
                i64::try_from(self.configuration_log.len()).unwrap() - 10,
                0,
            ))
            .unwrap();
            for req in &mut self.configuration_log[start..] {
                let (size, para) =
                    if matches!(self.ui_state.current_tab, super::SelectedTab::Configuration) {
                        req.paragraph()
                    } else {
                        req.paragraph_short()
                    };
                req_blocks.push(para);
                sum += size;
                constrains.push(Constraint::Length(size.try_into().unwrap()));
            }
            sum.try_into().unwrap_or(u16::MAX)
        };

        let width = if right_block_up.inner(area).height < total_length {
            // Make room for the scroll bar
            right_block_up.inner(area).width - 1
        } else {
            right_block_up.inner(area).width
        };

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
            self.ui_state.configuration_scroll_position(),
        );
        frame.render_widget(right_block_up, area);
    }

    fn render_user_input(&self, frame: &mut Frame, area: Rect) {
        let right_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("User Input")])
            .title_alignment(Alignment::Left);

        if self.user_input_manager.input_empty() {
            let mut text = Text::from(
                Span::from("Type a command, for example: ").patch_style(Style::new().dark_gray()),
            );
            text.push_span(
                Span::from(self.user_input_manager.command_name_list())
                    .patch_style(Style::new().dark_gray()),
            );
            let paragraph = Paragraph::new(text).block(right_block_down);
            frame.render_widget(paragraph, area);
            return;
        }
        let text: &str = &self.user_input_manager.user_input;
        let mut text = Text::from(text);

        let x_pos = area.x
            + min(
                u16::try_from(self.user_input_manager.user_input.len()).unwrap(),
                area.width,
            );

        frame.set_cursor_position(Position::new(x_pos + 1, area.y + 1));

        let (suggestion, cmds) = self.user_input_manager.suggestion();

        text.push_span(
            Span::from(
                suggestion
                    .get(self.user_input_manager.user_input.len()..)
                    .unwrap_or(""),
            )
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

        let (messages_height, paragraph) =
            if matches!(self.ui_state.current_tab, super::SelectedTab::Diagnostic) {
                self.diagnostic_log.paragraph()
            } else {
                self.diagnostic_log.paragraph_short()
            };
        let messages_height = messages_height.try_into().unwrap_or(u16::MAX);

        let mut scroll_view = ScrollView::new(Size::new(
            // Make room for the scroll bar
            content_width - 1,
            messages_height,
        ));

        scroll_view.render_widget(
            paragraph,
            Rect::new(0, 0, content_width - 1, messages_height),
        );

        frame.render_stateful_widget(
            scroll_view,
            left_block_up.inner(area),
            self.ui_state.diagnostic_scroll_position(),
        );
        frame.render_widget(left_block_up, area);
    }

    fn render_overall_messages(&mut self, frame: &mut Frame, area: Rect) {
        let left_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Diagnostic Messages")])
            .title_alignment(Alignment::Left);

        let content_width = left_block_up.inner(area).width;

        let (messages_height, paragraph) =
            if matches!(self.ui_state.current_tab, super::SelectedTab::Diagnostic) {
                self.overall_log.paragraph()
            } else {
                self.overall_log.paragraph_short()
            };
        let messages_height = messages_height.try_into().unwrap_or(u16::MAX);

        let mut scroll_view = ScrollView::new(Size::new(
            // Make room for the scroll bar
            content_width - 1,
            messages_height,
        ));

        scroll_view.render_widget(
            paragraph,
            Rect::new(0, 0, content_width - 1, messages_height),
        );

        frame.render_stateful_widget(
            scroll_view,
            left_block_up.inner(area),
            self.ui_state.diagnostic_scroll_position(),
        );
        frame.render_widget(left_block_up, area);
    }

    fn render_configuration_overview(&self, frame: &mut Frame, area: Rect) {
        let left_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Board Info")])
            .title_alignment(Alignment::Left);

        let text = self.ui_state.get_config();
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

        match self.ui_state.current_tab {
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
                self.render_overall_messages(frame, left_chunk_upper);
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
            super::SelectedTab::Commands => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(3)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_commands(frame, chunk_upper);
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
