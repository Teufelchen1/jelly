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
use ratatui::widgets::Wrap;
use ratatui::Frame;
use tui_widgets::scrollview::ScrollView;

use super::UiState;
use crate::datatypes::coap_log::CoapLog;
use crate::datatypes::diagnostic_log::DiagnosticLog;
use crate::datatypes::job_log::JobLog;
use crate::datatypes::packet_log::PacketLog;
use crate::datatypes::user_input_manager::InputType;
use crate::datatypes::user_input_manager::UserInputManager;

impl UiState {
    fn render_header_footer(&self, frame: &mut Frame, header_area: Rect, footer_area: Rect) {
        let tab_titles = [
            "Overview (F1)",
            "Text (F2)",
            "CoAP (F3)",
            "Commands (F4)",
            "Net (F5)",
            "Help (F12)",
        ];
        let title = if frame.area().width < 100 {
            "Jelly 🪼"
        } else {
            "Jelly 🪼: The friendly Shell for RIOT OS"
        };

        let tab_len: usize = tab_titles.map(|x| x.len() + 1).iter().sum();
        let title_area = Layout::horizontal([
            Constraint::Length(tab_len.try_into().unwrap()),
            Constraint::Fill(1),
        ])
        .split(header_area);

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

        let footer_title = self.get_connection();
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

        // Putting this here is not ideal, todo: somewhat autogenerate command list
        let text = include_str!("help.txt");
        let mut text = Text::from(text);
        let path = env::current_dir();
        match path {
            Ok(path) => {
                text.push_line(Line::from(format!(
                    "Your current working directory is: {:}\n",
                    path.display()
                )));
            }
            Err(_) => {
                text.push_line(Line::from("Your current working directory is unkown\n"));
            }
        }
        text.push_line(Line::from(""));
        text.push_line(Line::from("# Known commands\n"));
        text.extend(Text::from(self.command_help_list.clone()));

        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

        // Make room for the scroll bar
        let content_width = border_block.inner(area).width - 1;
        let messages_hight = u16::try_from(paragraph.line_count(content_width)).unwrap_or(u16::MAX);

        let mut scroll_view = ScrollView::new(Size::new(content_width, messages_hight));

        scroll_view.render_widget(paragraph, Rect::new(0, 0, content_width, messages_hight));

        frame.render_stateful_widget(
            scroll_view,
            border_block.inner(area),
            self.help_scroll.get_state_for_rendering(),
        );

        frame.render_widget(border_block, area);
    }

    fn render_network(&mut self, frame: &mut Frame, area: Rect, net_log: &PacketLog) {
        let right_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Network packets")])
            .title_alignment(Alignment::Left);

        let mut req_blocks = vec![];
        let mut constrains = vec![];
        let total_length: u16 = {
            let mut sum = 0;
            // temporay limitation to work around ratatui bug #1855
            let start =
                usize::try_from(max(i64::try_from(net_log.log().len()).unwrap() - 10, 0)).unwrap();
            for req in &net_log.log()[start..] {
                let (size, para) = req.paragraph();
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
            self.configuration_scroll.get_state_for_rendering(),
        );
        frame.render_widget(right_block_up, area);
    }

    fn render_commands(&mut self, frame: &mut Frame, area: Rect, job_log: &JobLog) {
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
                usize::try_from(max(i64::try_from(job_log.jobs.len()).unwrap() - 10, 0)).unwrap();
            for job in &job_log.jobs[start..] {
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
            self.command_scroll.get_state_for_rendering(),
        );
        frame.render_widget(right_block_up, area);
    }

    fn render_configuration_messages(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        configuration_log: &CoapLog,
        short: bool,
    ) {
        let right_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("CoAP Req & Resp")])
            .title_alignment(Alignment::Left);

        let (total_length, req_blocks, constrains) = configuration_log.to_paragraphs(short);

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
            self.configuration_scroll.get_state_for_rendering(),
        );
        frame.render_widget(right_block_up, area);
    }

    fn render_user_input(frame: &mut Frame, area: Rect, user_input_manager: &UserInputManager) {
        if user_input_manager.input_empty() {
            let right_block_down = Block::bordered()
                .border_style(Style::new().gray())
                .title(vec![Span::from("User Input")])
                .title_alignment(Alignment::Left);

            let mut text = Text::from(
                Span::from("Type a command, for example: ").patch_style(Style::new().dark_gray()),
            );
            text.push_span(
                Span::from(user_input_manager.command_name_list())
                    .patch_style(Style::new().dark_gray()),
            );
            let paragraph = Paragraph::new(text).block(right_block_down);
            frame.set_cursor_position(Position::new(area.x + 1, area.y + 1));
            frame.render_widget(paragraph, area);
            return;
        }
        let title = match user_input_manager.classify_input() {
            InputType::RawCoap(_) => "User Input: Raw CoAP request",
            InputType::RawCommand(_) => "User Input: Raw diagnostic command",
            InputType::Command(cmd, _, _) => &format!("User Input: {cmd}"),
        };
        let right_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from(title)])
            .title_alignment(Alignment::Left);

        let box_size: usize = usize::from(area.width.checked_sub(2).unwrap_or(1));
        let input_len = user_input_manager.user_input.len();
        let input = &user_input_manager.user_input;
        let mut text = Text::default();

        let mut start = 0;
        while start < input_len {
            let end = min(input_len, start + box_size);
            text.push_line(input.get(start..end).unwrap());
            start += box_size;
        }

        let y_pos =
            area.y + 1 + u16::try_from(user_input_manager.cursor_position / box_size).unwrap();
        let x_pos =
            area.x + 1 + u16::try_from(user_input_manager.cursor_position % box_size).unwrap();

        frame.set_cursor_position(Position::new(x_pos, y_pos));

        let (suggestion, cmds) = user_input_manager.suggestion();

        let mut completion_text = String::from(
            suggestion
                .get(user_input_manager.user_input.len()..)
                .unwrap_or(""),
        );
        if !cmds.is_empty() {
            completion_text.push_str(" | ");
            if cmds.len() == 1 {
                completion_text.push_str(&cmds[0].description);
            } else {
                let possible_commands = cmds
                    .iter()
                    .map(|x| x.cmd.clone())
                    .collect::<Vec<String>>()
                    .join(" ");
                completion_text.push_str(&possible_commands);
            }
        }

        // same as -input_len % box_size, but that would need cast to i64
        let remaining_space = (box_size - input_len % box_size) % box_size;

        if remaining_space < completion_text.len() {
            text.push_span(
                Span::from(&completion_text[..remaining_space])
                    .patch_style(Style::new().dark_gray()),
            );
            text.push_line(
                Span::from(&completion_text[remaining_space..])
                    .patch_style(Style::new().dark_gray()),
            );
        } else {
            text.push_span(Span::from(&completion_text).patch_style(Style::new().dark_gray()));
        }

        let paragraph = Paragraph::new(text).block(right_block_down);
        frame.render_widget(paragraph, area);
    }

    fn render_diagnostic_messages(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        diagnostic_log: &DiagnosticLog,
    ) {
        let left_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Text Messages")])
            .title_alignment(Alignment::Left);

        let content_width = left_block_up.inner(area).width;

        let (messages_height, paragraph) = diagnostic_log.paragraph();

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
            self.diagnostic_scroll.get_state_for_rendering(),
        );
        frame.render_widget(left_block_up, area);
    }

    fn render_overall_messages(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        overall_log: &DiagnosticLog,
    ) {
        let left_block_up = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Text & Commands")])
            .title_alignment(Alignment::Left);

        let content_width = left_block_up.inner(area).width;

        // Make room for the scroll bar
        let content_width = content_width - 1;

        let (_messages_height, paragraph) = overall_log.paragraph_short();
        let messages_height = paragraph.line_count(content_width);

        let messages_height = messages_height.try_into().unwrap_or(u16::MAX);

        let mut scroll_view = ScrollView::new(Size::new(content_width, messages_height));

        scroll_view.render_widget(paragraph, Rect::new(0, 0, content_width, messages_height));

        frame.render_stateful_widget(
            scroll_view,
            left_block_up.inner(area),
            self.overview_scroll.get_state_for_rendering(),
        );
        frame.render_widget(left_block_up, area);
    }

    fn render_configuration_overview(&self, frame: &mut Frame, area: Rect) {
        let left_block_down = Block::bordered()
            .border_style(Style::new().gray())
            .title(vec![Span::from("Board Info")])
            .title_alignment(Alignment::Left);

        let text = self.get_config();
        let text = Text::from(text);
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        let paragraph_block = paragraph.block(left_block_down);

        frame.render_widget(paragraph_block, area);
    }

    fn render_overview(
        &mut self,
        frame: &mut Frame,
        main_area: Rect,
        user_input_manager: &UserInputManager,
        configuration_log: &CoapLog,
        overall_log: &DiagnosticLog,
    ) {
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

        let configuration_log_area = right_chunks[0];
        let device_config_overview_area = right_chunks[1];

        let input_min_size = 3
            + (user_input_manager.user_input.len() + 10)
                .try_into()
                .unwrap_or(u16::MAX)
                / (main_chunk_left.width - 2);
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
            .split(main_chunk_left);

        let overall_messages_log_area = left_chunks[0];
        let userinput_area = left_chunks[1];

        self.render_configuration_messages(frame, configuration_log_area, configuration_log, true);
        self.render_configuration_overview(frame, device_config_overview_area);
        self.render_overall_messages(frame, overall_messages_log_area, overall_log);
        Self::render_user_input(frame, userinput_area, user_input_manager);
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        user_input_manager: &UserInputManager,
        job_log: &JobLog,
        logs: (&CoapLog, &DiagnosticLog, &DiagnosticLog, &PacketLog),
    ) {
        let (configuration_log, diagnostic_log, overall_log, net_log) = logs;

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

        let input_min_size = 3
            + (user_input_manager.user_input.len() + 10)
                .try_into()
                .unwrap_or(u16::MAX)
                / (main_area.width - 2);

        match self.current_tab {
            super::SelectedTab::Overview => {
                self.render_overview(
                    frame,
                    main_area,
                    user_input_manager,
                    configuration_log,
                    overall_log,
                );
            }
            super::SelectedTab::Diagnostic => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_diagnostic_messages(frame, chunk_upper, diagnostic_log);
                Self::render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Configuration => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_configuration_messages(frame, chunk_upper, configuration_log, false);
                Self::render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Commands => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_commands(frame, chunk_upper, job_log);
                Self::render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Net => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_network(frame, chunk_upper, net_log);
                Self::render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Help => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_help(frame, chunk_upper);
                Self::render_user_input(frame, chunk_lower, user_input_manager);
            }
        }
    }
}
