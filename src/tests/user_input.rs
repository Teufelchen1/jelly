#![allow(
    clippy::too_many_lines,
    reason = "Unittests can be repetitive and long, without downgrading the overall readability"
)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;

use crate::events::Event;

use super::AppTest;

#[test]
fn can_not_commit_input_when_not_connected() {
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
        "┌User Input: Raw diagnostic command───────────────────────┐│Board: Unknown               │",
        "│asdf                                                     ││                             │",
        "└─────────────────────────────────────────────────────────┘└─────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
    ];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}

#[test]
fn can_commit_input_when_connected() {
    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌Text & Commands──────────────────────────────────────────┐┌CoAP Req & Resp──────────────┐",
        "│                                                         ││ ← Req(Get /.well-known/core)│",
        "│                                                         ││Awaiting response            │",
        "│                                                         ││─────────────────────────────│",
        "│                                                         ││ ← Req(Get /jelly/board)     │",
        "│                                                         ││Awaiting response            │",
        "│                                                         ││─────────────────────────────│",
        "│                                                         ││ ← Req(Get /jelly/ver)       │",
        "│                                                         ││Awaiting response            │",
        "│                                                         ││─────────────────────────────│",
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
        "─────────────────────────────────────────────────────────────✅ connected via /dev/ttyUSB0",
    ]);

    let events = vec![
        Event::SerialConnect("/dev/ttyUSB0".to_owned()),
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
    ];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}

#[test]
fn autocomplete() {
    let mut test_app = AppTest::new();
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
        "┌User Input: Raw diagnostic command───────────────────────┐│Board: Unknown               │",
        "│Help | Help                                              ││                             │",
        "└─────────────────────────────────────────────────────────┘└─────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::Char('H'),
        KeyModifiers::empty(),
    ))];
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);

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
        "┌User Input: Help: Jelly Help─────────────────────────────┐│Board: Unknown               │",
        "│Help                                                     ││                             │",
        "└─────────────────────────────────────────────────────────┘└─────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::Tab,
        KeyModifiers::empty(),
    ))];
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);

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
    let events = vec![
        // move cursor one left Hel_p
        Event::TerminalKey(KeyEvent::new(KeyCode::Left, KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        // no matter how often we hit backspace, endresult is _p, moving cursor right to p_
        Event::TerminalKey(KeyEvent::new(KeyCode::Right, KeyModifiers::empty())),
        Event::TerminalKey(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        // Input should be empty now.
    ];
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}
