use crate::command::CommandLibrary;

pub enum SaveToFile {
    No,
    AsBin(String),
    AsText(String),
    ToStdout,
}

pub enum InputType {
    /// The user input something that is not known to Jelly but it
    /// starts with a `/` so it likely is a coap endpoint
    /// Treated as configuration message
    RawCoap(String),
    /// The user input something that is not known to Jelly
    /// Treated as diagnostic message
    RawCommand(String),
    /// This input is a known command
    Command(String, String, SaveToFile),
}

impl InputType {
    pub fn from_raw(user_input: &str, command_library: &CommandLibrary) -> Self {
        let (cmd_string, file) = if let Some((cmd_string, path)) = user_input.split_once("%>") {
            let path = path.trim();
            // To Stdout
            if path == "-" {
                (cmd_string, SaveToFile::ToStdout)
            } else {
                (cmd_string, SaveToFile::AsBin(path.to_owned()))
            }
        } else if let Some((cmd_string, path)) = user_input.split_once('>') {
            (cmd_string, SaveToFile::AsText(path.trim().to_owned()))
        } else {
            (user_input, SaveToFile::No)
        };
        let maybe_cmd = command_library.find_by_cmd(cmd_string.split(' ').next().unwrap());
        match maybe_cmd {
            Some(cmd) => Self::Command(cmd.cmd.clone(), cmd_string.to_owned(), file),
            None => {
                if user_input.starts_with('/') {
                    Self::RawCoap(user_input.to_owned())
                } else {
                    let mut cmd = user_input.to_owned();
                    if !cmd.ends_with('\n') {
                        cmd.push('\n');
                    }
                    Self::RawCommand(cmd)
                }
            }
        }
    }
}
