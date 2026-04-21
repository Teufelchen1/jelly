pub use coap_get_template::CoapGet;

use super::Command;
use super::CommandHandler;
use super::CommandType;

mod coap_get_template;
mod mem_read;
mod multi_endpoints_sample;
mod ps;
mod saul;
mod wkc;

mod force_cmds_available;
mod help;

#[derive(Debug)]
pub enum InternalCommand {
    Help(help::Help),
    ForceCmdsAvailable(force_cmds_available::ForceCmdsAvailable),
}

pub fn all_commands() -> Vec<Command> {
    vec![
        saul::cmd(),
        multi_endpoints_sample::cmd(),
        mem_read::cmd(),
        ps::cmd(),
        wkc::cmd(),
        Command::new_text_type("help", "Prints all available commands"),
        Command::new_coap_get("/.well-known/core", "Query the wkc"),
        help::cmd(),
        force_cmds_available::cmd(),
    ]
}
