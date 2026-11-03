use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;

use crate::events::Event;

use super::AppTest;

#[test]
fn returning_to_overview_tab_when_already_there_is_nop() {
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
        "└─────────────────────────────────────────────────────────┘│OS: Unknown                  │",
        "┌User Input───────────────────────────────────────────────┐│Board: Unknown               │",
        "│Type a command, for example: help, Help, ForceCmdsAvailab││                             │",
        "└─────────────────────────────────────────────────────────┘└─────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::F(1),
        KeyModifiers::empty(),
    ))];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}

#[test]
fn select_text_tab() {
    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌Text Messages───────────────────────────────────────────────────────────────────────────┐",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "┌User Input──────────────────────────────────────────────────────────────────────────────┐",
        "│Type a command, for example: help, Help, ForceCmdsAvailable                             │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::F(2),
        KeyModifiers::empty(),
    ))];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}

#[test]
fn select_coap_tab() {
    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌CoAP Req & Resp─────────────────────────────────────────────────────────────────────────┐",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "┌User Input──────────────────────────────────────────────────────────────────────────────┐",
        "│Type a command, for example: help, Help, ForceCmdsAvailable                             │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::F(3),
        KeyModifiers::empty(),
    ))];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}

#[test]
fn select_commands_tab() {
    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌Commands────────────────────────────────────────────────────────────────────────────────┐",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "┌User Input──────────────────────────────────────────────────────────────────────────────┐",
        "│Type a command, for example: help, Help, ForceCmdsAvailable                             │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::F(4),
        KeyModifiers::empty(),
    ))];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}

#[test]
fn select_net_tab() {
    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌Network packets─────────────────────────────────────────────────────────────────────────┐",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "│                                                                                        │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "┌User Input──────────────────────────────────────────────────────────────────────────────┐",
        "│Type a command, for example: help, Help, ForceCmdsAvailable                             │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::F(5),
        KeyModifiers::empty(),
    ))];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}

#[test]
fn select_help_tab() {
    let expected = Buffer::with_lines([
        "Overview (F1) Text (F2) CoAP (F3) Commands (F4) Net (F5) Help (F12) ───────Jelly 🪼───────",
        "┌Help────────────────────────────────────────────────────────────────────────────────────┐",
        "│Jelly 🪼: The friendly Shell for RIOT OS                                                ↑",
        "│                                                                                        █",
        "│Jelly is a tool that allows you to send commands to an attached device. The commands are█",
        "│send via slipmux and can be either plain text or CoAP based. A device is typically      █",
        "│connected via a serial (e.g. /dev/ttyUSB0) but Jelly also supports connecting via a unix█",
        "│socket, as long as the device speaks slipmux.                                           █",
        "│                                                                                        █",
        "│Hotkeys:                                                                                █",
        "│    Mousewheel: Scroll up and down.                                                     █",
        "│    F1: Presents an overview, showing diagnostic messages next to the configuration     ║",
        "│messages.                                                                               ║",
        "│    F2: Shows only the diagnostic messages.                                             ║",
        "│    F3: Shows only the configuration messages.                                          ║",
        "│    F4: Shows only the executed Host commands.                                          ║",
        "│    F5: This help :)                                                                    ║",
        "│    ESC: Quit Jelly.                                                                    ║",
        "│    RET: Send a command.                                                                ║",
        "│    TAB: Autocomplete.                                                                  ║",
        "│    UP: Go back in command history.                                                     ║",
        "│    DOWN: Go forward in command history.                                                ║",
        "│    LEFT: Move input cursor to the left.                                                ║",
        "│    RIGHT: Move input cursor to the right.                                              ║",
        "│    BACKSPACE: Delete a character left of the cursor.                                   ↓",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "┌User Input──────────────────────────────────────────────────────────────────────────────┐",
        "│Type a command, for example: help, Help, ForceCmdsAvailable                             │",
        "└────────────────────────────────────────────────────────────────────────────────────────┘",
        "──────────────────────────────────────────────────────────────❌ not connected, retrying..",
    ]);
    let events = vec![Event::TerminalKey(KeyEvent::new(
        KeyCode::F(12),
        KeyModifiers::empty(),
    ))];
    let mut test_app = AppTest::new();
    test_app.process_all_events(events);
    test_app.assert_eq(&expected);
}
