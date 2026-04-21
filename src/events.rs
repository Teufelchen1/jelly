use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;

use crate::command::InternalCommand;

#[derive(Debug)]
pub enum Event {
    AppCommand(InternalCommand),
    Diagnostic(String),
    Configuration(Vec<u8>),
    NetworkConnect(String),
    Packet(Vec<u8>),
    SendDiagnostic(String),
    SendConfiguration(Vec<u8>),
    SendPacket(Vec<u8>),
    SerialConnect(String),
    SerialDisconnect,
    TerminalString(String),
    TerminalKey(KeyEvent),
    TerminalMouse(MouseEvent),
    TerminalResize,
    TerminalEOF,
}
