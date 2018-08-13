extern crate flatbuffers;
extern crate futures;
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
mod fs;
pub mod handlers;

use libc::c_int;
use libc::c_void;
use std::collections::HashMap;
use std::env;
use std::ffi::CStr;
use std::ffi::CString;
use std::mem;

// Returns args passed to V8, followed by args passed to JS
fn parse_core_args(args: Vec<String>) -> (Vec<String>, Vec<String>) {
  let mut rest = vec![];

  // Filter out args that shouldn't be passed to V8
  let mut args: Vec<String> = args
    .into_iter()
    .filter(|arg| {
      if arg.as_str() == "--help" {
        rest.push(arg.clone());
        return false;
      }

      true
    })
    .collect();

  // Replace args being sent to V8
  for idx in 0..args.len() {
    if args[idx] == "--v8-options" {
      mem::swap(args.get_mut(idx).unwrap(), &mut String::from("--help"));
    }
  }

  (args, rest)
}

// Pass the command line arguments to v8.
// Returns a vector of command line arguments that v8 did not understand.
fn set_flags(args: Vec<String>) -> Vec<String> {
  // deno_set_flags(int* argc, char** argv) mutates argc and argv to remove
  // flags that v8 understands.
  // First parse core args, then converto to a vector of C strings.
  let (argv, rest) = parse_core_args(args);
  let mut argv = argv
    .iter()
    .map(|arg| CString::new(arg.as_str()).unwrap().into_bytes_with_nul())
    .collect::<Vec<_>>();

  // Make a new array, that can be modified by V8::SetFlagsFromCommandLine(),
  // containing mutable raw pointers to the individual command line args.
  let mut c_argv = argv
    .iter_mut()
    .map(|arg| arg.as_mut_ptr() as *mut i8)
    .collect::<Vec<_>>();
  // Store the length of the argv array in a local variable. We'll pass a
  // pointer to this local variable to deno_set_flags(), which then
  // updates its value.
  let mut c_argc = c_argv.len() as c_int;
  // Let v8 parse the arguments it recognizes and remove them from c_argv.
  unsafe {
    binding::deno_set_flags(&mut c_argc, c_argv.as_mut_ptr());
  };
  // If c_argc was updated we have to change the length of c_argv to match.
  c_argv.truncate(c_argc as usize);
  // Copy the modified arguments list into a proper rust vec and return it.
  c_argv
    .iter()
    .map(|ptr| unsafe {
      let cstr = CStr::from_ptr(*ptr as *const i8);
      let slice = cstr.to_str().unwrap();
      slice.to_string()
    })
    .chain(rest.into_iter())
    .collect()
}

type DenoException<'a> = &'a str;

pub struct Deno {
  ptr: *const binding::DenoC,
  dir: deno_dir::DenoDir,
  rt: tokio::runtime::current_thread::Runtime,
  timers: HashMap<u32, futures::sync::oneshot::Sender<()>>,
  argv: Vec<String>,
}

static DENO_INIT: std::sync::Once = std::sync::ONCE_INIT;

impl Deno {
  fn new(argv: Vec<String>) -> Box<Deno> {
    DENO_INIT.call_once(|| {
      unsafe { binding::deno_init() };
    });

    let mut deno_box = Box::new(Deno {
      ptr: 0 as *const binding::DenoC,
      dir: deno_dir::DenoDir::new(None).unwrap(),
      rt: tokio::runtime::current_thread::Runtime::new().unwrap(),
      timers: HashMap::new(),
      argv,
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

#[test]
fn test_parse_core_args_1() {
  let js_args =
    parse_core_args(vec!["deno".to_string(), "--v8-options".to_string()]);
  assert!(js_args == (vec!["deno".to_string(), "--help".to_string()], vec![]));
}

#[test]
fn test_parse_core_args_2() {
  let js_args = parse_core_args(vec!["deno".to_string(), "--help".to_string()]);
  assert!(js_args == (vec!["deno".to_string()], vec!["--help".to_string()]));
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
      println!("{} - {}", record.level(), record.args());
    }
  }
  fn flush(&self) {}
}

fn main() {
  log::set_logger(&LOGGER).unwrap();
  log::set_max_level(log::LevelFilter::Info);

  let js_args = set_flags(env::args().collect());

  /*
    let v = unsafe { deno_v8_version() };
    let c_str = unsafe { CStr::from_ptr(v) };
    let version = c_str.to_str().unwrap();
    println!("version: {}", version);
    */

  let mut d = Deno::new(js_args);

  d.execute("deno_main.js", "denoMain();")
    .unwrap_or_else(|err| {
      error!("{}", err);
      std::process::exit(1);
    });

  // Start the Tokio event loop
  d.rt.run().expect("err");
}
