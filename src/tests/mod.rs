use std::sync::mpsc::{self, Receiver, Sender};

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    symbols,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    app::App,
    events::Event,
    tui::{ProcessEventResult, UiState, process_next_event},
};

mod tab_handling;
mod user_input;

struct AppTest {
    app: App,
    ui_state: UiState,
    terminal: Terminal<TestBackend>,
    slipmux_event_sender: Sender<Event>,
    _slipmux_event_receiver: Receiver<Event>,
    event_sender: Sender<Event>,
    event_receiver: Receiver<Event>,
}

impl AppTest {
    pub fn new() -> Self {
        let mut ui_state = UiState::new(&crate::tui::ColorTheme::None);
        let mut terminal = Terminal::new(TestBackend::new(90, 30)).unwrap();

        let (event_sender, event_receiver) = mpsc::channel();
        let (slipmux_event_sender, slipmux_event_receiver) = mpsc::channel();

        terminal.clear().unwrap();

        let app = App::new(event_sender.clone());

        terminal
            .draw(|frame| app.draw(&mut ui_state, frame))
            .unwrap();

        Self {
            app,
            ui_state,
            terminal,
            slipmux_event_sender,
            _slipmux_event_receiver: slipmux_event_receiver,
            event_sender,
            event_receiver,
        }
    }

    pub fn send_event(&self, event: Event) {
        self.event_sender.send(event).unwrap();
    }

    pub fn process_all_events(&mut self, events: Vec<Event>) {
        for event in events {
            self.send_event(event);
            while let Ok(event) = self.event_receiver.try_recv() {
                match process_next_event(
                    &mut self.app,
                    &mut self.ui_state,
                    event,
                    &self.slipmux_event_sender,
                    None,
                ) {
                    ProcessEventResult::NothingToDo => panic!(),
                    ProcessEventResult::Ok => {
                        if self.ui_state.is_dirty() {
                            self.terminal
                                .draw(|frame| self.app.draw(&mut self.ui_state, frame))
                                .unwrap();
                            self.ui_state.wash();
                        }
                    }
                    ProcessEventResult::Terminate => panic!(),
                }
            }
        }
    }

    pub fn assert_eq(&self, expected: &Buffer) {
        let actual = self.terminal.backend().buffer();

        // Adopted from ratatui-core/src/buffer/assert.rs, thanks!
        assert!(
            actual.area == expected.area,
            "buffer areas not equal\nexpected: {expected:?}\nactual:   {actual:?}",
        );
        let nice_diff = expected
            .diff(actual)
            .into_iter()
            .enumerate()
            .map(|(i, (x, y, cell))| {
                let expected_cell = &expected[(x, y)];
                format!("{i}: at ({x}, {y})\n  expected: {expected_cell:?}\n  actual:   {cell:?}")
            })
            .collect::<Vec<String>>()
            .join("\n");
        assert!(
            nice_diff.is_empty(),
            "buffer contents not equal\nexpected: {expected:?}\nactual:   {actual:?}\ndiff:\n{nice_diff}",
        );
    }
}

fn run_events_in_app(events: Vec<Event>) -> Terminal<TestBackend> {
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.terminal
}

#[test]
fn example_how_to_test_widgets() {
    let backend = TestBackend::new(8, 2);
    let mut terminal = Terminal::new(backend).unwrap();

    let expected = Buffer::with_lines(["Hello  │", &format!("World! {}", symbols::line::VERTICAL)]);

    terminal
        .draw(|f| {
            let para = Paragraph::new("Hello World!\n")
                .wrap(Wrap { trim: false })
                .block(Block::new().borders(Borders::RIGHT));
            f.render_widget(para, Rect::new(0, 0, 8, 2));
        })
        .unwrap();
    terminal.backend().assert_buffer(&expected);
}

#[test]
fn app_tui_start() {
    let terminal = run_events_in_app(vec![Event::Diagnostic("Hello World\n".to_owned())]);

    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌Text & Commands──────────────────────────────────────────┐┌CoAP Req & Resp──────────────┐",
        "│Hello World                                              ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         │└─────────────────────────────┘",
        "│                                                         │┌Board Info───────────────────┐",
        "└─────────────────────────────────────────────────────────┘│Version: Unknown             │",
        "┌User Input───────────────────────────────────────────────┐│Board: Unknown               │",
        "│Type a command, for example: help, Help, ForceCmdsAvailab││                             │",
        "└─────────────────────────────────────────────────────────┘└─────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    terminal.backend().assert_buffer(&expected);
}

#[test]
#[should_panic(expected = "buffer contents not equal")]
fn app_tui_failed() {
    let terminal = run_events_in_app(vec![Event::Diagnostic("Hello World\n".to_owned())]);

    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌Text & Commands──────────────────────────────────────────┐┌CoAP Req & Resp──────────────┐",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         ││                             │",
        "│                                                         │└─────────────────────────────┘",
        "│                                                         │┌Board Info───────────────────┐",
        "└─────────────────────────────────────────────────────────┘│Version: Unknown             │",
        "┌User Input───────────────────────────────────────────────┐│Board: Unknown               │",
        "│Type a command, for example: help, Help, ForceCmdsAvailab││                             │",
        "└─────────────────────────────────────────────────────────┘└─────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    terminal.backend().assert_buffer(&expected);
}
