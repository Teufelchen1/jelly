use std::collections::HashMap;
use std::sync::mpsc::Sender;

use coap_lite::CoapOption;
use coap_lite::CoapRequest;
use coap_lite::Packet;
use coap_lite::RequestType as Method;
use datatypes::DiagnosticLog;
use datatypes::JobLog;
use rand::Rng;
use ratatui::Frame;
use slipmux::encode_buffered;
use slipmux::Slipmux;

use crate::app::datatypes::Job;
use crate::app::datatypes::Request;
use crate::app::datatypes::SaveToFile;
use crate::app::user_input_manager::InputType;
use crate::app::user_input_manager::UserInputManager;
use crate::events::Event;
use crate::tui::UiState;

pub mod datatypes;
mod handler;
pub mod user_input_manager;

pub struct App {
    connected: bool,
    event_sender: Sender<Event>,
    configuration_log: Vec<Request>,
    configuration_packets: Vec<Packet>,
    diagnostic_log: DiagnosticLog,
    user_input_manager: UserInputManager,
    ui_state: UiState,
    token_count: u16,
    next_mid: u16,
    overall_log: DiagnosticLog,
    ongoing_jobs: HashMap<u64, usize>,
    job_log: JobLog,
}

impl App {
    pub fn new(event_sender: Sender<Event>) -> Self {
        Self {
            connected: false,
            event_sender,

            configuration_log: vec![],
            configuration_packets: vec![],
            diagnostic_log: DiagnosticLog::new(),

            user_input_manager: UserInputManager::new(),

            ui_state: UiState::new(),

            token_count: 0,
            next_mid: rand::rng().random(),

            overall_log: DiagnosticLog::new(),
            ongoing_jobs: HashMap::new(),
            job_log: JobLog::new(),
        }
    }

    pub fn force_all_commands_availabe(&mut self) {
        self.user_input_manager.force_all_commands_availabe();
    }

    pub fn unfinished_jobs_count(&self) -> usize {
        self.ongoing_jobs.len()
    }

    pub fn dump_job_log(&self) -> Vec<String> {
        self.job_log.dump()
    }

    fn get_new_token(&mut self) -> Vec<u8> {
        self.token_count += 1;
        self.token_count.to_le_bytes().to_vec()
    }

    const fn get_new_message_id(&mut self) -> u16 {
        self.next_mid = self.next_mid.wrapping_add(1);
        self.next_mid
    }

    fn build_get_request(path: &str) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(path);
        request
    }

    fn send_configuration_request(&mut self, msg: &mut Packet) {
        msg.header.message_id = self.get_new_message_id();
        msg.set_token(self.get_new_token());
        msg.add_option(CoapOption::Block2, vec![0x05]);

        let data = encode_buffered(Slipmux::Configuration(msg.to_bytes().unwrap()));
        self.event_sender
            .send(Event::SendConfiguration(data))
            .unwrap();
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        self.ui_state.draw(
            frame,
            &self.user_input_manager,
            &self.job_log,
            &self.configuration_log,
            &self.diagnostic_log,
            &self.overall_log,
        );
    }
}
