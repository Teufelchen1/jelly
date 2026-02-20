use std::fs;
use std::path::Path;

pub use coap_get_template::CoapGet;

use super::Command;
use super::CommandHandler;
use super::CommandType;

mod coap_get_template;
mod mem_read;
mod multi_endpoints_sample;
mod ps;
mod roto_template;
mod saul;
mod wkc;

fn find_roto_commands(path: &Path) -> Vec<Command> {
    let mut result = vec![];
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            result.extend(find_roto_commands(&path));
        } else {
            result.push(roto_template::cmd(entry.file_name().to_str().unwrap()));
        }
    }
    result
}

pub fn all_commands() -> Vec<Command> {
    let mut all_cmds = vec![
        saul::cmd(),
        multi_endpoints_sample::cmd(),
        mem_read::cmd(),
        ps::cmd(),
        wkc::cmd(),
        Command::new_text_type("help", "Prints all available commands"),
        Command::new_coap_get("/.well-known/core", "Query the wkc"),
        Command::new_jelly_type("Help", "Jelly Help"),
        Command::new_jelly_type(
            "ForceCmdsAvailable",
            "Enables all implemented commands disregarding their requirements",
        ),
    ];
    all_cmds.extend(find_roto_commands(Path::new("./roto_commands")));
    all_cmds
}
