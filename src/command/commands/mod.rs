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
            cmd: "NyanCat".to_string(),
            description: "Fills your input field with nekos".to_string(),
            required_endpoints: vec![],
            parse: |_c, _a| Ok(CommandType::Jelly),
        },
    ]
}
