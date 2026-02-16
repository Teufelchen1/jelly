use std::sync::mpsc::{self, Sender};

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

mod basic;

fn start_tui_for_testing(event_sender: Sender<Event>) -> (App, UiState, Terminal<TestBackend>) {
    let mut ui_state = UiState::new(&crate::tui::ColorTheme::None);

    let mut terminal = Terminal::new(TestBackend::new(90, 30)).unwrap();

    terminal.clear().unwrap();

    let app = App::new(event_sender);

    terminal
        .draw(|frame| app.draw(&mut ui_state, frame))
        .unwrap();

    (app, ui_state, terminal)
}

fn run_events_in_app(events: Vec<Event>) -> Terminal<TestBackend> {
    let (event_sender, event_receiver) = mpsc::channel();
    let (slipmux_event_sender, _slipmux_event_receiver) = mpsc::channel();

    let (mut app, mut ui_state, mut terminal) = start_tui_for_testing(event_sender.clone());

    for event in events {
        event_sender.send(event).unwrap();

        match process_next_event(
            &mut app,
            &mut ui_state,
            &event_receiver,
            &slipmux_event_sender,
            None,
        ) {
            ProcessEventResult::NothingToDo => panic!(),
            ProcessEventResult::Ok => {
                if ui_state.is_dirty() {
                    terminal
                        .draw(|frame| app.draw(&mut ui_state, frame))
                        .unwrap();
                }
            }
            ProcessEventResult::Terminate => panic!(),
        }
    }

    terminal
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
