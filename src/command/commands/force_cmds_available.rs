use crate::app::App;
use crate::tui::UiState;

use super::Command;
use super::CommandType;
use super::InternalCommand;

pub type ForceCmdsAvailable = fn(&mut App, Option<&mut UiState>);

pub fn cmd() -> Command {
    Command {
        cmd: "ForceCmdsAvailable".to_owned(),
        description: "Enables all implemented commands disregarding their requirements".to_owned(),
        parse: |_, _| Ok(CommandType::Jelly(InternalCommand::ForceCmdsAvailable(run))),
        required_endpoints: vec![],
    }
}

pub fn run(app: &mut App, ui_state: Option<&mut UiState>) {
    app.force_all_commands_availabe(ui_state);
}
