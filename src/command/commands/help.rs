use crate::app::App;
use crate::tui::UiState;

use super::Command;
use super::CommandType;
use super::InternalCommand;

pub type Help = fn(&App, Option<&mut UiState>);

pub fn cmd() -> Command {
    Command {
        cmd: "Help".to_owned(),
        description: "Jelly Help".to_owned(),
        parse: |_, _| Ok(CommandType::Jelly(InternalCommand::Help(run))),
        required_endpoints: vec![],
    }
}

fn run(app: &App, ui_state: Option<&mut UiState>) {
    if let Some(ui_state) = ui_state {
        ui_state.select_help_view();
        app.populate_command_help_list(ui_state);
    }
}
