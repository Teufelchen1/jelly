//! Utility wrapper that enables commands to not implement the current [`CommandHandler`] trait,
//! but instead run an async task.
//!
//! This module is laden with impedance mismatches, blocking in unsuitable places and even memory
//! leaks. While the latter would be avoidable with some added complexity, most others are a
//! consequence of the blocking main interface. This module's deeper purpose is not to provide full
//! functionality of an async handler, but to allow writing some first async handlers and to gain
//! experience with them.

use std::sync::LazyLock;

use super::Command;
use super::CommandHandler;

/// All components that a typical command implementation needs if it uses the `asynch` module.
pub mod prelude {
    pub use super::MainProgram;
    pub use std::fmt::Write as _;
}

type CoapRequest = coap_lite::CoapRequest<String>;

// Yes the type is a monstrosity -- but that monstrosity is not inherent to using async code, just
// to doing simple async (where futures are not necessarily Send) when the main thread is blocking:
// Without access to the executor (which can not be sent), we have to move proto-stages of the
// task-to-be around, with type erasure because we can't access generics through a channel, so that
// eventually they can be spawned inside the right thread.
type ToBeSpawned = Box<dyn Send + FnOnce() -> Box<dyn std::future::Future<Output = ()>>>;
static TO_EXECUTOR: LazyLock<async_channel::Sender<ToBeSpawned>> = LazyLock::new(|| {
    let (to_exec, from_main) = async_channel::unbounded::<ToBeSpawned>();
    std::thread::spawn(move || {
        let ex = smol::LocalExecutor::new();
        futures_lite::future::block_on(ex.run(async {
            loop {
                let Ok(next) = from_main.recv().await else {
                    // Sender went away; shut down thread cleanly.
                    break;
                };
                let task = ex.spawn(Box::into_pin(next())).await;
                // FIXME store in Handler::task rather than leaking. That'd also help with eventual
                // cleanup if there were ever cancelled commands.
                Box::leak(Box::new(task));
            }
        }))
    });
    to_exec
});

/// A representative of the main thread in an async command task.
///
/// Note that while this offers a few methods, not all are currently supported in any order or take
/// effect immediately -- for example, any prints with delays would be spooled up until the first
/// CoAP request is sent.
pub struct MainProgram {
    to_main: async_channel::Sender<TaskToMain>,
    from_main: async_channel::Receiver<MainToTask>,
}

// Not implementing a pub write function -- writeln! is easy to use.
impl std::fmt::Write for MainProgram {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.to_main.try_send(TaskToMain::Text(s.into()))
            .expect("Main task dropped receiver before dropping the task. (Queue is unlimited, we could send anything immediately.)");
        Ok(())
    }
}

impl MainProgram {
    pub async fn request(&mut self, request: CoapRequest) -> coap_lite::Packet {
        self.to_main
            .send(TaskToMain::Request(request))
            .await
            .expect("Main task dropped receiver before dropping the task.");
        match self
            .from_main
            .recv()
            .await
            .expect("Main task dropped receiver before dropping the task.")
        {
            MainToTask::Response(packet) => packet,
        }
    }
}

pub trait AsyncCommand {
    const COMMAND: &str;
    const DESCRIPTION: &str;

    async fn run(args: &str, main: MainProgram) -> Result<(), impl core::fmt::Display>;
}

/// Creates a (regular) command from an asynchronous command ([`AsyncCommand`]) implementation.
pub fn build<C>(_cmd: C) -> Command
where
    C: AsyncCommand,
{
    // FIXME: We're only really using C as a marker type; can't pass in any instance properties
    // because we can't convey the instance through the returned Command (parse is a pure function,
    // not a closure)
    Command {
        cmd: C::COMMAND.to_string(),
        description: C::DESCRIPTION.to_string(),
        required_endpoints: vec!["/time".into()], // FIXME: No clue
        parse: parse::<C>,
    }
}

/// Kicks off an instance of a command.
fn parse<C>(_cmd: &Command, args: &str) -> Result<Box<dyn CommandHandler>, String>
where
    C: AsyncCommand,
{
    let (to_main, from_task) = async_channel::unbounded();
    let (to_task, from_main) = async_channel::unbounded();
    let to_main_for_error = to_main.clone();

    let args = args.to_owned();

    let task = TO_EXECUTOR
        .try_send(Box::new(|| {
            Box::new(async move {
                match C::run(&args, MainProgram { to_main, from_main }).await {
                    Ok(()) => (),
                    Err(e) => {
                        let error_text = format!("Error executing command: {e}");
                        // Discarding any error: if this can not be sent, the command has already been
                        // aborted by the main application.
                        let _ = to_main_for_error.send(TaskToMain::Text(error_text)).await;
                    }
                }
            })
        }))
        .expect("Executor crashed");

    let mut enqueued_to_print = std::collections::VecDeque::new();

    // As the task could stop before it produces a request, i.e. err at start, and as we can't
    // print errors in init(), we have to block here until the first request is ready

    let request = loop {
        let msg = smol::block_on(from_task.recv());
        match msg {
            Ok(TaskToMain::Text(t)) => enqueued_to_print.push_back(t),
            Ok(TaskToMain::Request(r)) => break Some(r),
            Err(_) => {
                if enqueued_to_print.is_empty() {
                    enqueued_to_print
                        .push_back("Command failed and provided no error details.".to_string());
                }
                break None;
            }
        }
    };

    if let Some(request) = request {
        Ok(Box::new(Handler {
            task,
            from_task,
            to_task,
            enqueued_to_print: std::cell::RefCell::new(enqueued_to_print),
            enqueued_request: Some(request),
        }))
    } else {
        // not cheapest but simplest way to join as per https://stackoverflow.com/a/73853277
        Err(Vec::from(enqueued_to_print).join("\n"))
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum TaskToMain {
    Text(String),
    Request(CoapRequest),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum MainToTask {
    Response(coap_lite::Packet),
}

struct Handler /*<C: AsyncCommand>*/ {
    task: (), // FIXME: Should be something like smol::Task<()> -- once the handler goes away, we
    // should drop/cancel the task.
    from_task: async_channel::Receiver<TaskToMain>,
    to_task: async_channel::Sender<MainToTask>,
    // RefCell gives us inner mutability, so we can pop items during display(&self)
    enqueued_to_print: std::cell::RefCell<std::collections::VecDeque<String>>,
    // One needs to be enqueued at .init()
    enqueued_request: Option<CoapRequest>,
}

impl CommandHandler for Handler {
    fn init(&mut self) -> CoapRequest {
        self.enqueued_request
            .take()
            .expect("Request must be enqueued before handing off to .init()")
    }

    fn handle(&mut self, payload: &coap_lite::Packet) -> Option<coap_lite::CoapRequest<String>> {
        // If this fails, that's because the task dropped its receiver. Unless it is trying to be
        // deliberately obnoxious, it'll also have dropped its sender. Continuing in order to flush
        // out any remaining messages it might have sent into the enqueued prints; when the queue
        // is empty and the sender is gone, it'll `.ok()?` there.
        let _ = self.to_task.try_send(MainToTask::Response(payload.clone()));

        loop {
            let msg = smol::block_on(self.from_task.recv())
                // see above
                .ok()?;
            match msg {
                TaskToMain::Text(t) => self.enqueued_to_print.borrow_mut().push_back(t),
                TaskToMain::Request(r) => break Some(r),
            }
        }
    }

    fn want_display(&self) -> bool {
        !self.enqueued_to_print.borrow().is_empty()
    }

    fn is_finished(&self) -> bool {
        // FIXME: Should we test aliveness of the receiving socket here?
        false
    }

    fn display(&self, buffer: &mut String) {
        use std::fmt::Write;
        while let Some(p) = self.enqueued_to_print.borrow_mut().pop_front() {
            buffer
                .write_str(&p)
                .expect("Writing to a string never fails");
        }
    }

    fn export(&self) -> Vec<u8> {
        let mut buffer = String::new();
        self.display(&mut buffer);
        buffer.as_bytes().to_vec()
    }
}
