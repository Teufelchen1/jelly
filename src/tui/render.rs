use std::cmp::min;
use std::env;
use std::iter::zip;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarState;
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

use super::UiState;
use super::scrolling::get_areas_to_render_from_scroll_position;
use crate::datatypes::coap_log::CoapLog;
use crate::datatypes::coap_log::Request;
use crate::datatypes::diagnostic_log::DiagnosticLog;
use crate::datatypes::job_log::Job;
use crate::datatypes::job_log::JobLog;
use crate::datatypes::packet_log::PacketDirection;
use crate::datatypes::packet_log::PacketLog;
use crate::datatypes::user_input_manager::InputType;
use crate::datatypes::user_input_manager::UserInputManager;

impl UiState {
    fn render_scrollbar(
        frame: &mut Frame,
        area: Rect,
        scroll_position: usize,
        max_scroll_offset: usize,
    ) {
        let scrollbar = Scrollbar::default()
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll_offset).position(max_scroll_offset - scroll_position);
        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
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
                .highlight_style(self.selected_style())
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
        let outer_block = Block::bordered()
            .border_style(self.border_style())
            .title(vec![Span::from("Help")])
            .title_alignment(Alignment::Left);

        let viewport_height = outer_block.inner(area).height as usize;
        let content_width = outer_block.inner(area).width;

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
        let messages_height = paragraph.line_count(content_width);

        let max_scroll_offset = messages_height.saturating_sub(viewport_height);

        self.help_scroll.last_max_position = max_scroll_offset;

        if self.help_scroll.follow {
            self.help_scroll.position = max_scroll_offset;
        }
        let scroll_offset = self.help_scroll.position;

        let paragraph = paragraph.scroll((scroll_offset.try_into().unwrap_or(u16::MAX), 0));

        let block = paragraph.block(outer_block);

        frame.render_widget(block, area);

        // More content than fits on the screen? Show scrollbar
        if messages_height > viewport_height {
            Self::render_scrollbar(
                frame,
                area,
                max_scroll_offset - scroll_offset,
                max_scroll_offset,
            );
        }
    }

    fn get_representation_of_network<'a>(
        &self,
        packet: &'a PacketDirection,
    ) -> (usize, Paragraph<'a>) {
        let block = Block::new()
            .borders(Borders::BOTTOM)
            .style(self.border_style())
            .title_alignment(Alignment::Left)
            .title(packet.get_title());
        let (height, para) = packet.paragraph();
        let para = para.block(block).style(Style::reset());
        (height + 2, para)
    }

    fn render_network(&mut self, frame: &mut Frame, area: Rect, net_log: &PacketLog) {
        let outer_block = Block::bordered()
            .border_style(self.border_style())
            .title(vec![Span::from("Network packets")])
            .title_alignment(Alignment::Left);

        let viewport_height = outer_block.inner(area).height as usize;

        let mut height_log = vec![];
        for packet in net_log.log() {
            let (size, _) = packet.paragraph();
            let size = size + 2;
            height_log.push(size);
        }

        let total_height: usize = height_log.iter().sum();
        let max_scroll_offset = total_height.saturating_sub(viewport_height);

        self.net_scroll.last_max_position = max_scroll_offset;

        if self.net_scroll.follow {
            self.net_scroll.position = max_scroll_offset;
        }
        let scroll_offset = max_scroll_offset - self.net_scroll.position;

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(
                outer_block.inner(area),
                scroll_offset,
                &height_log,
            );

        if let Some((index, area)) = partial_draw_top {
            let packet = &net_log.log()[index];
            let (height, para) = self.get_representation_of_network(packet);

            let buffer_area = Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
            let mut buffer = Buffer::empty(buffer_area);

            para.render(buffer_area, &mut buffer);

            let visible_content = buffer
                .content
                .into_iter()
                .skip(area.width as usize * (height - area.height as usize))
                .take(area.area() as usize);
            for (i, cell) in visible_content.enumerate() {
                let x = i as u16 % area.width;
                let y = i as u16 / area.width;
                frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
            }
        }

        if let Some((index, area)) = partial_draw_bottom {
            let packet = &net_log.log()[index];
            let (height, para) = self.get_representation_of_network(packet);

            let buffer_area = Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
            let mut buffer = Buffer::empty(buffer_area);

            para.render(buffer_area, &mut buffer);

            let visible_content = buffer.content.into_iter().take(area.area() as usize);
            for (i, cell) in visible_content.enumerate() {
                let x = i as u16 % area.width;
                let y = i as u16 / area.width;
                frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
            }
        }

        if let Some((range, area)) = full_draw_middle {
            let mut job_blocks = vec![];
            let mut constrains = vec![];
            for packet in &net_log.log()[range] {
                let (height, para) = self.get_representation_of_network(packet);
                job_blocks.push(para);
                constrains.push(Constraint::Length(height.try_into().unwrap()));
            }

            let areas: Vec<Rect> = Layout::vertical(constrains).split(area).to_vec();
            for (a, packet) in zip(areas, job_blocks) {
                packet.render(a, frame.buffer_mut());
            }
        }

        frame.render_widget(&outer_block, area);

        // More content than fits on the screen? Show scrollbar
        if total_height > viewport_height {
            Self::render_scrollbar(frame, area, scroll_offset, max_scroll_offset);
        }
    }

    fn get_representation_of_job<'a>(&self, job: &'a Job) -> (usize, Paragraph<'a>) {
        let block = Block::new()
            .borders(Borders::BOTTOM)
            .style(self.border_style())
            .title_alignment(Alignment::Left)
            .title(job.get_title());
        let (height, para) = job.paragraph();
        let para = para.block(block).style(Style::reset());
        (height + 2, para)
    }

    fn render_commands(&mut self, frame: &mut Frame, area: Rect, job_log: &JobLog) {
        let outer_block = Block::bordered()
            .border_style(self.border_style())
            .title(vec![Span::from("Commands")])
            .title_alignment(Alignment::Left);

        let viewport_height = outer_block.inner(area).height as usize;

        let mut height_log = vec![];
        for job in &job_log.jobs {
            let (size, _) = job.paragraph();
            let size = size + 2;
            height_log.push(size);
        }

        let total_height: usize = height_log.iter().sum();
        let max_scroll_offset = total_height.saturating_sub(viewport_height);

        self.command_scroll.last_max_position = max_scroll_offset;

        if self.command_scroll.follow {
            self.command_scroll.position = max_scroll_offset;
        }
        let scroll_offset = max_scroll_offset - self.command_scroll.position;

        let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
            get_areas_to_render_from_scroll_position(
                outer_block.inner(area),
                scroll_offset,
                &height_log,
            );

        if let Some((index, area)) = partial_draw_top {
            let job = &job_log.jobs[index];
            let (height, para) = self.get_representation_of_job(job);

            let buffer_area = Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
            let mut buffer = Buffer::empty(buffer_area);

            para.render(buffer_area, &mut buffer);

            let visible_content = buffer
                .content
                .into_iter()
                .skip(area.width as usize * (height - area.height as usize))
                .take(area.area() as usize);
            for (i, cell) in visible_content.enumerate() {
                let x = i as u16 % area.width;
                let y = i as u16 / area.width;
                frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
            }
        }

        if let Some((index, area)) = partial_draw_bottom {
            let job = &job_log.jobs[index];
            let (height, para) = self.get_representation_of_job(job);

            let buffer_area = Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
            let mut buffer = Buffer::empty(buffer_area);

            para.render(buffer_area, &mut buffer);

            let visible_content = buffer.content.into_iter().take(area.area() as usize);
            for (i, cell) in visible_content.enumerate() {
                let x = i as u16 % area.width;
                let y = i as u16 / area.width;
                frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
            }
        }

        if let Some((range, area)) = full_draw_middle {
            let mut job_blocks = vec![];
            let mut constrains = vec![];
            for job in &job_log.jobs[range] {
                let (height, para) = self.get_representation_of_job(job);
                job_blocks.push(para);
                constrains.push(Constraint::Length(height.try_into().unwrap()));
            }

            let areas: Vec<Rect> = Layout::vertical(constrains).split(area).to_vec();
            for (a, job) in zip(areas, job_blocks) {
                job.render(a, frame.buffer_mut());
            }
        }

        frame.render_widget(&outer_block, area);

        // More content than fits on the screen? Show scrollbar
        if total_height > viewport_height {
            Self::render_scrollbar(frame, area, scroll_offset, max_scroll_offset);
        }
    }

    fn get_representation_of_configuration_message<'a>(
        &self,
        req: &'a Request,
        short: bool,
    ) -> (usize, Paragraph<'a>) {
        let block = Block::new()
            .borders(Borders::BOTTOM)
            .style(self.border_style())
            .title_alignment(Alignment::Left);

        let (height, para) = if short {
            let block = block.title(vec![Span::from(req.get_request_title_short())]);
            let (height, para) = req.paragraph_short();
            (height, para.block(block))
        } else {
            let block = block.title(vec![Span::from(req.get_request_title())]);
            let (height, para) = req.paragraph();
            (height, para.block(block))
        };
        (height + 2, para)
    }

    fn render_configuration_messages(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        configuration_log: &mut CoapLog,
        short: bool,
    ) {
        let outer_block = Block::bordered()
            .border_style(self.border_style())
            .title(vec![Span::from("CoAP Req & Resp")])
            .title_alignment(Alignment::Left);

        let viewport_height = outer_block.inner(area).height as usize;
        let viewport_width = outer_block.inner(area).width;
        let mut total_height = 0;
        let mut max_scroll_offset = 0;
        let mut scroll_offset = 0;

        let mut update_needed = true;

        let mut height_log = configuration_log
            .get_render_memo_for_width(viewport_width)
            .to_vec();

        'outer: while update_needed {
            update_needed = false;

            total_height = height_log.iter().sum();
            max_scroll_offset = total_height.saturating_sub(viewport_height);

            self.configuration_scroll.last_max_position = max_scroll_offset;

            if self.configuration_scroll.follow {
                self.configuration_scroll.position = max_scroll_offset;
            }
            scroll_offset = max_scroll_offset - self.configuration_scroll.position;

            let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
                get_areas_to_render_from_scroll_position(
                    outer_block.inner(area),
                    scroll_offset,
                    &height_log,
                );

            if let Some((index, area)) = partial_draw_top {
                let req = &configuration_log.requests[index];
                let (height, para) = self.get_representation_of_configuration_message(&req, short);

                if height != height_log[index] {
                    height_log[index] = height;
                    update_needed = true;
                } else {
                    let buffer_area =
                        Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
                    let mut buffer = Buffer::empty(buffer_area);

                    para.render(buffer_area, &mut buffer);

                    let visible_content = buffer
                        .content
                        .into_iter()
                        .skip(area.width as usize * (height - area.height as usize))
                        .take(area.area() as usize);
                    for (i, cell) in visible_content.enumerate() {
                        let x = i as u16 % area.width;
                        let y = i as u16 / area.width;
                        frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
                    }
                }
            }

            if let Some((index, area)) = partial_draw_bottom {
                let req = &configuration_log.requests[index];
                let (height, para) = self.get_representation_of_configuration_message(req, short);

                if height != height_log[index] {
                    height_log[index] = height;
                    update_needed = true;
                } else {
                    let buffer_area =
                        Rect::new(0, 0, area.width, height.try_into().unwrap_or(u16::MAX));
                    let mut buffer = Buffer::empty(buffer_area);

                    para.render(buffer_area, &mut buffer);

                    let visible_content = buffer.content.into_iter().take(area.area() as usize);
                    for (i, cell) in visible_content.enumerate() {
                        let x = i as u16 % area.width;
                        let y = i as u16 / area.width;
                        frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
                    }
                }
            }

            if let Some((range, area)) = full_draw_middle {
                let mut req_blocks = vec![];
                let mut constrains = vec![];
                for index in range {
                    let req = &configuration_log.requests[index];
                    let (height, para) =
                        self.get_representation_of_configuration_message(req, short);

                    if height != height_log[index] {
                        height_log[index] = height;
                        update_needed = true;
                        continue 'outer;
                    }

                    req_blocks.push(para);
                    constrains.push(Constraint::Length(height.try_into().unwrap()));
                }

                let areas: Vec<Rect> = Layout::vertical(constrains).split(area).to_vec();
                for (a, req_b) in zip(areas, req_blocks) {
                    req_b.render(a, frame.buffer_mut());
                }
            }
        }

        // Update the cache
        configuration_log
            .render_memo_cache
            .insert(viewport_width, height_log);

        frame.render_widget(&outer_block, area);

        // More content than fits on the screen? Show scrollbar
        if total_height > viewport_height {
            Self::render_scrollbar(frame, area, scroll_offset, max_scroll_offset);
        }
    }

    fn render_user_input(
        &self,
        frame: &mut Frame,
        area: Rect,
        user_input_manager: &UserInputManager,
    ) {
        if user_input_manager.input_empty() {
            let right_block_down = Block::bordered()
                .border_style(self.border_style())
                .title(vec![Span::from("User Input")])
                .title_alignment(Alignment::Left);

            let mut text = Text::from(
                Span::from("Type a command, for example: ").patch_style(self.downlight()),
            );
            text.push_span(
                Span::from(user_input_manager.command_name_list()).patch_style(self.downlight()),
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
            .border_style(self.border_style())
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
                Span::from(&completion_text[..remaining_space]).patch_style(self.downlight()),
            );
            text.push_line(
                Span::from(&completion_text[remaining_space..]).patch_style(self.downlight()),
            );
        } else {
            text.push_span(Span::from(&completion_text).patch_style(self.downlight()));
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
        let outer_block = Block::bordered()
            .border_style(self.border_style())
            .title(vec![Span::from("Text Messages")])
            .title_alignment(Alignment::Left);

        let viewport_height = outer_block.inner(area).height as usize;
        let content_width = outer_block.inner(area).width;

        let (_, paragraph) = diagnostic_log.paragraph();
        let messages_height = paragraph.line_count(content_width);

        let max_scroll_offset = messages_height.saturating_sub(viewport_height);
        self.diagnostic_scroll.last_max_position = max_scroll_offset;

        if self.diagnostic_scroll.follow {
            self.diagnostic_scroll.position = max_scroll_offset;
        }
        let scroll_offset = self.diagnostic_scroll.position;

        let paragraph = paragraph.scroll((scroll_offset.try_into().unwrap_or(u16::MAX), 0));

        let block = paragraph.block(outer_block);

        frame.render_widget(block, area);

        // More content than fits on the screen? Show scrollbar
        if messages_height > viewport_height {
            Self::render_scrollbar(
                frame,
                area,
                max_scroll_offset - scroll_offset,
                max_scroll_offset,
            );
        }
    }

    fn render_overall_messages(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        overall_log: &DiagnosticLog,
    ) {
        let outer_block = Block::bordered()
            .border_style(self.border_style())
            .title(vec![Span::from("Text & Commands")])
            .title_alignment(Alignment::Left);

        let viewport_height = outer_block.inner(area).height as usize;
        let content_width = outer_block.inner(area).width;

        let (_, paragraph) = overall_log.paragraph_short();
        let messages_height = paragraph.line_count(content_width);

        let max_scroll_offset = messages_height.saturating_sub(viewport_height);

        self.overview_scroll.last_max_position = max_scroll_offset;

        if self.overview_scroll.follow {
            self.overview_scroll.position = max_scroll_offset;
        }
        let scroll_offset = self.overview_scroll.position;

        let paragraph = paragraph.scroll((scroll_offset.try_into().unwrap_or(u16::MAX), 0));

        let block = paragraph.block(outer_block);

        frame.render_widget(block, area);

        // More content than fits on the screen? Show scrollbar
        if messages_height > viewport_height {
            Self::render_scrollbar(
                frame,
                area,
                max_scroll_offset - scroll_offset,
                max_scroll_offset,
            );
        }
    }

    fn render_configuration_overview(&self, frame: &mut Frame, area: Rect) {
        let left_block_down = Block::bordered()
            .border_style(self.border_style())
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
        configuration_log: &mut CoapLog,
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
        self.render_user_input(frame, userinput_area, user_input_manager);
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        user_input_manager: &UserInputManager,
        job_log: &JobLog,
        logs: (&mut CoapLog, &DiagnosticLog, &DiagnosticLog, &PacketLog),
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
                self.render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Configuration => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_configuration_messages(frame, chunk_upper, configuration_log, false);
                self.render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Commands => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_commands(frame, chunk_upper, job_log);
                self.render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Net => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_network(frame, chunk_upper, net_log);
                self.render_user_input(frame, chunk_lower, user_input_manager);
            }
            super::SelectedTab::Help => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(input_min_size)].as_ref())
                    .split(main_area);

                let chunk_upper = chunks[0];
                let chunk_lower = chunks[1];

                self.render_help(frame, chunk_upper);
                self.render_user_input(frame, chunk_lower, user_input_manager);
            }
        }
    }
}

// Without optimization 18.74s
// With optimization 5.35s
// #[cfg(test)]
// mod tests {
//     use coap_lite::CoapRequest;
//     use coap_lite::Packet;
//     use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

//     use super::*;
//     use crate::tui::App;
//     use std::sync::mpsc;

//     #[test]
//     fn render_test() {
//         let mut ui_state = UiState::new();

//         let mut frame = Frame {
//             cursor_position: None,
//             viewport_area: Rect {
//                 x: 0,
//                 y: 0,
//                 width: 200,
//                 height: 13,
//             },
//             buffer: &mut Buffer::empty(Rect {
//                 x: 0,
//                 y: 0,
//                 width: 200,
//                 height: 13,
//             }),
//             count: 1,
//         };
//         let (event_sender, _event_receiver) = mpsc::channel();
//         let mut app = App::new(event_sender.clone());
//         let mut coap_req: CoapRequest<String> = CoapRequest::new();
//         let mut coap_res: Packet = Packet::new();
//         coap_res.payload = vec![0xa, 0xc, 0xa, 0xb];
//         let mut token_count: u32 = 1;
//         for _ in 0..1000 {
//             token_count += 1;
//             coap_req
//                 .message
//                 .set_token(token_count.to_le_bytes().to_vec());

//             app.configuration_log.push(coap_req.clone());
//             app.configuration_log
//                 .requests
//                 .last_mut()
//                 .unwrap()
//                 .add_response(coap_res.clone());
//         }

//         app.on_mouse(MouseEvent {
//             kind: crossterm::event::MouseEventKind::ScrollDown,
//             column: 0,
//             row: 0,
//             modifiers: KeyModifiers::empty(),
//         });
//         app.on_key(
//             Some(&mut ui_state),
//             KeyEvent::new(KeyCode::F(3), KeyModifiers::empty()),
//         );
//         for _ in 0..1000 {
//             app.draw(&mut ui_state, &mut frame);
//         }

//         for _ in 0..1000 {
//             token_count += 1;
//             coap_req
//                 .message
//                 .set_token(token_count.to_le_bytes().to_vec());

//             app.configuration_log.push(coap_req.clone());
//             app.configuration_log
//                 .requests
//                 .last_mut()
//                 .unwrap()
//                 .add_response(coap_res.clone());
//         }

//         for _ in 0..1000 {
//             app.draw(&mut ui_state, &mut frame);
//         }
//     }
// }
