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
        Command {
            cmd: "Help".to_owned(),
            description: "Jelly Help".to_owned(),
            required_endpoints: vec![],
            parse: |_c, _a| Ok(CommandType::Jelly),
        },
        Command {
            cmd: "ForceCmdsAvailable".to_owned(),
            description: "Enables all implemented commands disregarding their requirements"
                .to_owned(),
            required_endpoints: vec![],
            parse: |_c, _a| Ok(CommandType::Jelly),
        },
    ]
}
