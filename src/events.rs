use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;

pub enum Event {
    Diagnostic(String),
    Configuration(Vec<u8>),
    Packet(Vec<u8>),
    SendDiagnostic(String),
    SendConfiguration(Vec<u8>),
    SerialConnect(String),
    SerialDisconnect,
    TerminalString(String),
    TerminalKey(KeyEvent),
    TerminalMouse(MouseEvent),
    TerminalResize,
}
