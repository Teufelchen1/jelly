use std::fmt::Write;
use std::ops::DerefMut;
use std::path::Path;
use std::sync::Arc;

use coap_lite::{CoapRequest, MessageClass, Packet};
use coap_lite::{RequestType as Method, ResponseType};
use roto::{List, NoCtx, Runtime, TypedFunc, Val, library};

use super::Command;
use super::CommandHandler;
use super::CommandType;

struct Roto {
    runtime: Runtime<NoCtx>,
    roto_init: TypedFunc<NoCtx, fn() -> Val<CoapRequest<String>>>,
    roto_handle: Option<TypedFunc<NoCtx, fn(Val<Packet>) -> Option<Val<CoapRequest<String>>>>>,
    roto_want_display: Option<TypedFunc<NoCtx, fn() -> bool>>,
    roto_is_finished: Option<TypedFunc<NoCtx, fn() -> bool>>,
    roto_display: Option<TypedFunc<NoCtx, fn() -> Arc<str>>>,
    buffer: String,
}

pub fn cmd(name: &str) -> Command {
    Command {
        cmd: name.to_owned(),
        description: "Run a Roto Script".to_owned(),
        parse: |c, a| Ok(CommandType::CoAP(parse(c, a))),
        required_endpoints: vec![],
    }
}

fn parse(cmd: &Command, _args: &str) -> Box<dyn CommandHandler> {
    let lib = library! {
        #[clone] type CoapRequest = Val<CoapRequest<String>>;
        #[copy] type Response = Val<ResponseType>;
        #[clone] type Packet = Val<Packet>;

        impl Val<CoapRequest<String>> {
            fn new() -> Self {
                Val(CoapRequest::new())
            }

            fn set_method(mut self) {
                self.deref_mut().set_method(Method::Get);
            }

            fn set_path(mut self, path: Arc<str>) -> Val<CoapRequest<String>> {
                self.deref_mut().set_path(&path);
                self
            }
        }

        impl Val<Packet> {
            fn get_payload(self) -> List<u8> {
                List::from(self.payload.clone())
            }

            fn is_error(self) -> bool {
                if let MessageClass::Response(response) = self.header.code {
                    response.is_error()
                } else {
                    true
                }
            }

            // fn get_response_type(self) -> Val<ResponseType> {
            //     if let MessageClass::Response(response) = self.header.code {
            //         Val(response)
            //     } else {
            //         panic!();
            //     }
            // }
        }

        fn new_request() -> Val<CoapRequest<String>> {
            let mut request: CoapRequest<String> = CoapRequest::new();
            request.set_method(Method::Get);
            roto::Val(request)
        }
    };
    let mut runtime = Runtime::from_lib(lib).unwrap();
    runtime.add_io_functions();

    let result = runtime.compile(Path::new("./roto_commands/").join(Path::new(&cmd.cmd)));
    let mut pkg = match result {
        Ok(pkg) => pkg,
        Err(err) => {
            panic!("{err}");
        }
    };

    let roto_init = pkg
        .get_function::<fn() -> Val<CoapRequest<String>>>("init")
        .unwrap();
    let roto_handle = pkg
        .get_function::<fn(Val<Packet>) -> Option<Val<CoapRequest<String>>>>("handle")
        .ok();
    let roto_want_display = pkg.get_function::<fn() -> bool>("want_display").ok();
    let roto_is_finished = pkg.get_function::<fn() -> bool>("is_finished").ok();
    let roto_display = pkg.get_function::<fn() -> Arc<str>>("display").ok();

    Box::new(Roto {
        runtime,
        roto_init,
        roto_handle,
        roto_want_display,
        roto_is_finished,
        roto_display,
        buffer: String::new(),
    })
}

impl CommandHandler for Roto {
    fn init(&mut self) -> CoapRequest<String> {
        let res = &self.roto_init.call();
        res.0.clone()
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        if let Some(roto_handle) = &self.roto_handle {
            if let Some(req) = roto_handle.call(Val(response.clone())) {
                Some(req.0)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn want_display(&self) -> bool {
        if let Some(roto_want_display) = &self.roto_want_display {
            roto_want_display.call()
        } else {
            false
        }
    }

    fn is_finished(&self) -> bool {
        if let Some(roto_is_finished) = &self.roto_is_finished {
            roto_is_finished.call()
        } else {
            true
        }
    }

    fn display(&self, buffer: &mut String) {
        if let Some(roto_display) = &self.roto_display {
            let _ = write!(buffer, "{} | {}", roto_display.call(), self.buffer);
        } else {
            let _ = writeln!(buffer, "{}", self.buffer);
        }
    }
}
