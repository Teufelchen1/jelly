use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

use clap::Parser;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use minicbor::Decoder;

use super::Command;
use super::CommandHandler;
use super::CommandRegistry;

/// This is an example on how to use cbor as payload for the coap request.
#[derive(Parser, Debug)]
#[command(name = "Ps")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is ps over coap")]
struct PsCli {}

pub struct Ps {
    location: String,
    buffer: String,
    payload: Vec<u8>,
    finished: bool,
    displayable: bool,
}

impl CommandRegistry for Ps {
    fn cmd() -> Command {
        Command {
            cmd: "Ps".to_owned(),
            description: "Ps over coap, print thread info".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/jelly/Ps".to_owned()],
        }
    }

    fn parse(cmd: &Command, args: String) -> Result<Box<dyn CommandHandler>, String> {
        let _cli = PsCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
        Ok(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            payload: vec![],
            finished: false,
            displayable: false,
        }))
    }
}

enum ThreadStatus {
    Stopped,        // has terminated
    Zombie,         // has terminated & keeps thread's thread_t
    Sleeping,       // sleeping
    MutexBlocked,   // waiting for a locked mutex
    ReceiveBlocked, // waiting for a message
    SendBlocked,    // waiting for message to be delivered
    ReplyBlocked,   // waiting for a message response
    FlagBlockedAny, // waiting for any flag from flag_mask
    FlagBlockedAll, // waiting for all flags in flag_mask
    MboxBlocked,    // waiting for get/put on mbox
    CondBlocked,    // waiting for a condition variable
    Running,        // currently running
    Pending,        // waiting to be scheduled to run
    Numof,          // number of supported thread states
}

impl ThreadStatus {
    const fn from_u32(data: u32) -> Self {
        match data {
            0 => Self::Stopped,
            1 => Self::Zombie,
            2 => Self::Sleeping,
            3 => Self::MutexBlocked,
            4 => Self::ReceiveBlocked,
            5 => Self::SendBlocked,
            6 => Self::ReplyBlocked,
            7 => Self::FlagBlockedAny,
            8 => Self::FlagBlockedAll,
            9 => Self::MboxBlocked,
            10 => Self::CondBlocked,
            11 => Self::Running,
            12 => Self::Pending,
            _ => Self::Numof,
        }
    }
}

impl Display for ThreadStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let s = match self {
            Self::Stopped => "stopped".to_owned(),
            Self::Zombie => "zombie".to_owned(),
            Self::Sleeping => "sleeping".to_owned(),
            Self::MutexBlocked => "bl mutex".to_owned(),
            Self::ReceiveBlocked => "bl rx".to_owned(),
            Self::SendBlocked => "bl send".to_owned(),
            Self::ReplyBlocked => "bl reply".to_owned(),
            Self::FlagBlockedAny => "bl anyfl".to_owned(),
            Self::FlagBlockedAll => "bl allfl".to_owned(),
            Self::MboxBlocked => "bl mbox".to_owned(),
            Self::CondBlocked => "bl cond".to_owned(),
            Self::Running => "running".to_owned(),
            Self::Pending => "pending".to_owned(),
            Self::Numof => "numof".to_owned(),
        };
        write!(f, "{s}")
    }
}

impl CommandHandler for Ps {
    fn init(&mut self) -> CoapRequest<String> {
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Get);
        request.set_path(&self.location);
        request
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        self.payload = payload.to_vec();
        let mut out = String::new();
        let mut decoder = Decoder::new(payload);
        let mut sum_stack = 0;
        let mut sum_stack_used = 0;
        let mut sum_stack_free = 0;

        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{:^3} | {:^20} | {:^8} {:^1} | {:^3} | {:>6} {:>6} {:>6} | {:^10} | {:^10}",
            "pid", "name", "state", "Q", "pri", "stack", "(used)", "(free)", "base addr", "sp"
        );

        // First one is ISR, not at all brittle code, lol
        decoder.array().unwrap();
        {
            decoder.array().unwrap();
            let stacksize = decoder.u32().unwrap();
            let stackusage = decoder.u32().unwrap();
            let stackfree = decoder.u32().unwrap();
            let base_addr = decoder.u32().unwrap();
            let current_addr = decoder.u32().unwrap();

            let _ = writeln!(
                out,
                "{:>3} | {:<20} | {:^8} {:^1} | {:>3} | {:>6} {:>6} {:>6} | {:#010x} | {:#010x}",
                "-",
                "isr_stack",
                "-",
                "-",
                "-",
                stacksize,
                stackusage,
                stackfree,
                base_addr,
                current_addr
            );

            sum_stack += stacksize;
            sum_stack_used += stackusage;
            sum_stack_free += stackfree;
        }

        // Second is the list of threads
        decoder.array().unwrap();
        {
            while decoder.probe().array().is_ok() {
                decoder.array().unwrap();
                let pid = decoder.u8().unwrap();
                let name = decoder.str().unwrap();
                let state = decoder.u32().unwrap();
                let state = ThreadStatus::from_u32(state).to_string();
                let active = decoder.bool().unwrap();
                let active = if active { "✔" } else { "✕" };
                let prio = decoder.u32().unwrap();
                let stacksize = decoder.u32().unwrap();
                let stackusage = decoder.u32().unwrap();
                let stackfree = decoder.u32().unwrap();
                let base_addr = decoder.u32().unwrap();
                let current_addr = decoder.u32().unwrap();

                #[allow(clippy::uninlined_format_args)]
                let _ = writeln!(
                    out,
                    "{:>3} | {:<20} | {:<8} {:^1} | {:>3} | {:>6} {:>6} {:>6} | {:#010x} | {:#010x}",
                    pid,
                    name,
                    state,
                    active,
                    prio,
                    stacksize,
                    stackusage,
                    stackfree,
                    base_addr,
                    current_addr
                );

                sum_stack += stacksize;
                sum_stack_used += stackusage;
                sum_stack_free += stackfree;
            }
        }

        let _ = writeln!(
            out,
            "{:>3} | {:<20} | {:^8} {:^1} | {:>3} | {:>6} {:>6} {:>6}",
            " ", "SUM", " ", " ", " ", sum_stack, sum_stack_used, sum_stack_free,
        );

        self.buffer = out;
        self.finished = true;
        self.displayable = true;
        None
    }

    fn want_display(&self) -> bool {
        self.displayable
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn display(&self, buffer: &mut String) {
        let _ = writeln!(buffer, "{}", self.buffer);
    }

    fn export(&self) -> Vec<u8> {
        self.payload.clone()
    }
}
