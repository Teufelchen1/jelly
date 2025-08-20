pub use coap_get_template::CoapGet;
use mem::MemRead;
use multi_endpoints_sample::MultiEndpointSample;
use sample::SampleCommand;
use saul::Saul;
pub use wkc::Wkc;

use super::Command;
use super::CommandHandler;
use super::CommandRegistry;

mod coap_get_template;
mod mem;
mod multi_endpoints_sample;
mod sample;
mod saul;
mod wkc;

pub fn all_commands() -> Vec<Command> {
    vec![
        SampleCommand::cmd(),
        Saul::cmd(),
        MultiEndpointSample::cmd(),
        MemRead::cmd(),
    ]
}
