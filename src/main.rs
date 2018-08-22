extern crate flatbuffers;
extern crate futures;
extern crate hyper;
extern crate libc;
extern crate msg_rs as msg_generated;
extern crate sha1;
extern crate tempfile;
extern crate tokio;
extern crate tokio_current_thread;
extern crate url;
#[macro_use]
extern crate log;

mod binding;
mod deno_dir;
mod errors;
mod flags;
mod fs;
pub mod handlers;
mod version;

use libc::c_void;
use std::collections::HashMap;
use std::env;
use std::ffi::CStr;
use std::ffi::CString;

type DenoException<'a> = &'a str;

pub struct Deno {
  ptr: *const binding::DenoC,
  dir: deno_dir::DenoDir,
  rt: tokio::runtime::current_thread::Runtime,
  timers: HashMap<u32, futures::sync::oneshot::Sender<()>>,
  argv: Vec<String>,
  flags: flags::DenoFlags,
}

static DENO_INIT: std::sync::Once = std::sync::ONCE_INIT;

impl Deno {
  fn new(argv: Vec<String>) -> Box<Deno> {
    DENO_INIT.call_once(|| {
      unsafe { binding::deno_init() };
    });

    let (flags, argv_rest) = flags::set_flags(argv);

    let mut deno_box = Box::new(Deno {
      ptr: 0 as *const binding::DenoC,
      dir: deno_dir::DenoDir::new(None).unwrap(),
      rt: tokio::runtime::current_thread::Runtime::new().unwrap(),
      timers: HashMap::new(),
      argv: argv_rest,
      flags,
    });

    (*deno_box).ptr = unsafe {
      binding::deno_new(
        deno_box.as_ref() as *const _ as *const c_void,
        handlers::msg_from_js,
      )
    };

    deno_box
  }

  fn execute(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), DenoException> {
    let filename = CString::new(js_filename).unwrap();
    let source = CString::new(js_source).unwrap();
    let r = unsafe {
      binding::deno_execute(self.ptr, filename.as_ptr(), source.as_ptr())
    };
    if r == 0 {
      let ptr = unsafe { binding::deno_last_exception(self.ptr) };
      let cstr = unsafe { CStr::from_ptr(ptr) };
      return Err(cstr.to_str().unwrap());
    }
    Ok(())
  }
}

impl Drop for Deno {
  fn drop(&mut self) {
    unsafe { binding::deno_delete(self.ptr) }
  }
}

pub fn from_c<'a>(d: *const binding::DenoC) -> &'a mut Deno {
  let ptr = unsafe { binding::deno_get_data(d) };
  let deno_ptr = ptr as *mut Deno;
  let deno_box = unsafe { Box::from_raw(deno_ptr) };
  Box::leak(deno_box)
}

#[test]
fn test_c_to_rust() {
  let argv = vec![String::from("./deno"), String::from("hello.js")];
  let d = Deno::new(argv);
  let d2 = from_c(d.ptr);
  assert!(d.ptr == d2.ptr);
  assert!(d.dir.root.join("gen") == d.dir.gen, "Sanity check");
}

static LOGGER: Logger = Logger;

struct Logger;

impl log::Log for Logger {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    metadata.level() <= log::max_level()
  }

  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      println!("{} RS - {}", record.level(), record.args());
    }
  }
  fn flush(&self) {}
}

fn main() {
  log::set_logger(&LOGGER).unwrap();

  let js_args = flags::v8_set_flags(env::args().collect());

  let mut d = Deno::new(js_args);

  if d.flags.help {
    flags::print_usage();
    std::process::exit(0);
  }

  if d.flags.version {
    version::print_version();
    std::process::exit(0);
  }

  log::set_max_level(if d.flags.log_debug {
    log::LevelFilter::Debug
  } else {
    log::LevelFilter::Info
  });

  d.execute("deno_main.js", "denoMain();")
    .unwrap_or_else(|err| {
      error!("{}", err);
      std::process::exit(1);
    });

  // Start the Tokio event loop
  d.rt.run().expect("err");
}
