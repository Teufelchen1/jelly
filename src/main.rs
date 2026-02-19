use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use clap::Parser;

use crate::events::Event;
use crate::headless::start_headless_configuration;
use crate::headless::start_headless_diagnostic;
use crate::headless::start_headless_diagnostic_network;
use crate::headless::start_headless_network;
use crate::network::create_network_thread;
use crate::slipmux::create_slipmux_thread;
use crate::tui::ColorTheme;
use crate::tui::start_tui;

mod app;
mod command;
mod datatypes;
mod events;
mod headless;
mod network;
mod slipmux;
mod transport;
mod tui;

#[cfg(test)]
mod tests;

type EventChannel = (Sender<Event>, Receiver<Event>);

#[derive(Parser)]
struct Cli {
    /// The path to the UART TTY interface
    tty_path: std::path::PathBuf,

    /// If enabled, creates a SLIP network interface
    /// Optionally specifies the base name of the interface
    ///
    /// Requires higher privileges to create the TUN interface.
    #[arg(short = 't', long, verbatim_doc_comment)]
    #[allow(clippy::option_option)]
    network: Option<Option<String>>,

    /// If true, disables the TUI and passes diagnostic messages via stdio
    ///
    /// This is interactive. Jelly will await input and output indefinitely.
    /// Configuration messages are ignored.
    /// This means that any pre-known or configuration-based commands are not
    /// available.
    #[arg(
        short = 'd',
        long,
        default_value_t = false,
        verbatim_doc_comment,
        conflicts_with = "headless_configuration"
    )]
    headless_diagnostic: bool,

    /// If true, disables the TUI and passes configuration messages via stdio
    ///
    /// Use this mode inside scripts and pipe commands into Jelly.
    /// This may be used interactive.
    /// Jelly will await input unitl EOF.
    /// Jelly will wait for output until all commands are finished or
    /// the time-out is reached. The output will only be displayed once EOF is
    /// reached. This is to preserve the order of input commands regardless of
    /// the commands run time.
    /// Diagnostic messages are ignored.
    /// Pre-known, configuration-based commands are available.
    #[arg(
        short = 'c',
        long,
        default_value_t = false,
        verbatim_doc_comment,
        conflicts_with_all = ["headless_diagnostic", "headless_network"]
    )]
    headless_configuration: bool,

    /// If true, disables the TUI and acts as a network interface only
    ///
    /// Configuration messages are ignored.
    #[arg(
        short = 'n',
        long,
        default_value_t = false,
        verbatim_doc_comment,
        conflicts_with = "headless_configuration"
    )]
    headless_network: bool,

    /// Sets the color theme of the Jelly TUI
    ///
    /// This setting has no effect when not using the TUI.
    #[arg(
        long,
        value_enum,
        default_value_t = ColorTheme::Auto,
        verbatim_doc_comment,
    )]
    color_theme: ColorTheme,
}

fn main() {
    let args = Cli::parse();
    if !args.tty_path.exists() {
        println!("{} could not be found.", args.tty_path.display());
        return;
    }

    let main_channel: EventChannel = mpsc::channel();

    if args.headless_diagnostic && (args.headless_network || args.network.is_some()) {
        start_headless_diagnostic_network(args, main_channel);
    } else if args.headless_diagnostic {
        start_headless_diagnostic(args, main_channel);
    } else if args.headless_network {
        start_headless_network(args, main_channel);
    } else if args.headless_configuration {
        start_headless_configuration(args, main_channel);
    } else {
        start_tui(args, main_channel);
        println!("Thank you for using Jelly 🪼");
    }
}
