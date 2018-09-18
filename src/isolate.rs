// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use deno_dir;
use flags;
use futures;
use handlers;
use libc::c_void;
use libdeno;
use std;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use tokio;

type DenoException<'a> = &'a str;

pub struct Isolate {
  pub ptr: *const libdeno::DenoC,
  pub dir: deno_dir::DenoDir,
  pub rt: tokio::runtime::current_thread::Runtime,
  pub timers: HashMap<u32, futures::sync::oneshot::Sender<()>>,
  pub argv: Vec<String>,
  pub flags: flags::DenoFlags,
}

static DENO_INIT: std::sync::Once = std::sync::ONCE_INIT;

impl Isolate {
  pub fn new(argv: Vec<String>) -> Box<Isolate> {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let (flags, argv_rest) = flags::set_flags(argv);

    let mut deno_box = Box::new(Isolate {
      ptr: 0 as *const libdeno::DenoC,
      dir: deno_dir::DenoDir::new(flags.reload, None).unwrap(),
      rt: tokio::runtime::current_thread::Runtime::new().unwrap(),
      timers: HashMap::new(),
      argv: argv_rest,
      flags,
    });

    (*deno_box).ptr = unsafe {
      libdeno::deno_new(
        deno_box.as_ref() as *const _ as *const c_void,
        handlers::msg_from_js,
      )
    };

    deno_box
  }

  pub fn execute(
    &self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), DenoException> {
    let filename = CString::new(js_filename).unwrap();
    let source = CString::new(js_source).unwrap();
    let r = unsafe {
      libdeno::deno_execute(self.ptr, filename.as_ptr(), source.as_ptr())
    };
    if r == 0 {
      let ptr = unsafe { libdeno::deno_last_exception(self.ptr) };
      let cstr = unsafe { CStr::from_ptr(ptr) };
      return Err(cstr.to_str().unwrap());
    }
    Ok(())
  }
}

impl Drop for Isolate {
  fn drop(&mut self) {
    unsafe { libdeno::deno_delete(self.ptr) }
  }
}

pub fn from_c<'a>(d: *const libdeno::DenoC) -> &'a mut Isolate {
  let ptr = unsafe { libdeno::deno_get_data(d) };
  let ptr = ptr as *mut Isolate;
  let isolate_box = unsafe { Box::from_raw(ptr) };
  Box::leak(isolate_box)
}

#[test]
fn test_c_to_rust() {
  let argv = vec![String::from("./deno"), String::from("hello.js")];
  let isolate = Isolate::new(argv);
  let isolate2 = from_c(isolate.ptr);
  assert_eq!(isolate.ptr, isolate2.ptr);
  assert_eq!(
    isolate.dir.root.join("gen"),
    isolate.dir.gen,
    "Sanity check"
  );
}
