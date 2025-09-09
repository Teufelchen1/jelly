pub use coap_get_template::CoapGet;
use mem::MemRead;
use multi_endpoints_sample::MultiEndpointSample;
use ps::Ps;
use saul::Saul;
pub use wkc::Wkc;

use super::Command;
use super::CommandHandler;
use super::CommandType;

mod coap_get_template;
mod mem;
mod multi_endpoints_sample;
mod ps;
mod saul;
mod wkc;

pub fn all_commands() -> Vec<Command> {
    vec![
        Saul::cmd(),
        MultiEndpointSample::cmd(),
        MemRead::cmd(),
        Ps::cmd(),
        Wkc::cmd(),
        Command::new_text_type("help", "Prints all available commands"),
        Command::new_coap_get("/.well-known/core", "Query the wkc"),
        Command::new_jelly_type("Help", "Jelly Help"),
        Command::new_jelly_type(
            "ForceCmdsAvailable",
            "Enables all implemented commands disregarding their requirements",
        ),
    ]
}
