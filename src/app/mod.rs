use std::sync::mpsc::Sender;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::Packet;
use coap_lite::RequestType as Method;
use rand::Rng;
use ratatui::text::Text;
use slipmux::encode_configuration;
use tui_widgets::scrollview::ScrollViewState;

use crate::app::commands::CommandLibrary;
use crate::events::Event;

mod commands;
mod handler;
mod tui;

pub struct App<'text> {
    event_sender: Sender<Event>,
    write_port: Option<String>,
    configuration_requests: Vec<CoapRequest<String>>,
    configuration_packets: Vec<Packet>,
    configuration_scroll_state: ScrollViewState,
    configuration_scroll_follow: bool,
    diagnostic_messages: Text<'text>,
    diagnostic_messages_scroll_state: ScrollViewState,
    diagnostic_messages_scroll_position: usize,
    diagnostic_messages_scroll_follow: bool,
    known_commands: CommandLibrary,
    user_input: String,
    user_command_history: Vec<String>,
    user_command_cursor: usize,
    token_count: u16,
    riot_board: String,
    riot_version: String,
    next_mid: u16,
}
impl App<'_> {
    pub fn new(event_sender: Sender<Event>) -> Self {
        Self {
            event_sender,
            write_port: None,
            configuration_requests: vec![],
            configuration_packets: vec![],
            configuration_scroll_state: ScrollViewState::default(),
            configuration_scroll_follow: true,
            diagnostic_messages: Text::default(),
            diagnostic_messages_scroll_state: ScrollViewState::default(),
            diagnostic_messages_scroll_position: 0,
            diagnostic_messages_scroll_follow: true,
            known_commands: CommandLibrary::default(),
            user_input: String::new(),
            user_command_history: vec![],
            user_command_cursor: 0,
            token_count: 0,
            riot_board: "Unkown".to_owned(),
            riot_version: "Unkown".to_owned(),

            next_mid: rand::rng().random(),
        }
    }

    fn get_new_token(&mut self) -> Vec<u8> {
        self.token_count += 1;
        self.token_count.to_le_bytes().to_vec()
    }

    const fn get_new_message_id(&mut self) -> u16 {
        self.next_mid = self.next_mid.wrapping_add(1);
        self.next_mid
    }

    fn build_request(&mut self, path: &str) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(path);
        request.message.header.message_id = self.get_new_message_id();
        request.message.set_token(self.get_new_token());
        request.message.add_option(CoapOption::Block2, vec![0x05]);
        request
    }

    fn send_configuration_request(&self, msg: &Packet) {
        let (data, size) = encode_configuration(msg.to_bytes().unwrap());
        self.event_sender
            .send(Event::SendConfiguration(data[..size].to_vec()))
            .unwrap();
    }

    fn _suggest_command(&self) -> Option<(&str, &str)> {
        let cmds = self.known_commands.matching_prefix_by_cmd(&self.user_input);
        let mut matching_cutoff = self.user_input.len();

        match cmds.len() {
            0 => {
                None
            },
            1 => {
                Some((&cmds[0].cmd, &cmds[0].description))
            },
            _ => {
                let first = &cmds[0].cmd;
                while matching_cutoff+1 < first.len() {
                    let (cmd_name, _rest) = first.split_at(matching_cutoff+1);
                    if !cmds.eq(&self.known_commands.matching_prefix_by_cmd(cmd_name)) {
                        break;
                    }
                    matching_cutoff += 1;
                }
                let suggestion = first.split_at(matching_cutoff).0;
                Some((suggestion, "Foo"))
            }
        }


        // if cmds.len() > 1 {
        //     let first_ = self.known_commands.matching_prefix_by_cmd(&self.user_input).first();
        //     if let Some(first) = first_ {
        //         while matching_cutoff+1 < first.cmd.len() {
        //             let cmd_name = first.cmd.split_at(matching_cutoff+1).0;
        //             if !cmds.eq(&self.known_commands.matching_prefix_by_cmd(cmd_name)) {
        //                 break;
        //             }
        //             matching_cutoff += 1;
        //         }
        //     }
        // if cmds.len() == 0 {

        // }
        // } else if cmds.len() == 1 {
        //     return Some(&cmds.pop().unwrap().cmd);
        // } else {

        // }


        // let first_ = self.known_commands.matching_prefix_by_cmd(&self.user_input).pop();
        // if let Some(first) = first_ {
        //     let second_ = self.known_commands.matching_prefix_by_cmd(&self.user_input).pop();
        //     if let Some(second) = second_ {

        //         let (left, right) = first.cmd.split_at(self.user_input.len()+1);

        //     }
        // }
    }
}
