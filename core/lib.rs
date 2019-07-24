// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate log;
extern crate futures;
extern crate libc;

mod any_error;
mod flags;
mod isolate;
mod js_errors;
mod libdeno;
mod module_specifier;
mod modules;
mod shared_queue;

pub use crate::any_error::*;
pub use crate::flags::v8_set_flags;
pub use crate::isolate::*;
pub use crate::js_errors::*;
pub use crate::libdeno::deno_mod;
pub use crate::libdeno::OpId;
pub use crate::libdeno::PinnedBuf;
pub use crate::module_specifier::*;
pub use crate::modules::*;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub fn v8_version() -> &'static str {
  use std::ffi::CStr;
  let version = unsafe { libdeno::deno_v8_version() };
  let c_str = unsafe { CStr::from_ptr(version) };
  c_str.to_str().unwrap()
}

// TODO(mtharrison): Move these somewhere else

#[derive(Clone)]
pub struct InspectorHandle {
  pub tx: Arc<Mutex<Sender<String>>>,
  pub rx: Arc<Mutex<Receiver<String>>>,
}

impl InspectorHandle {
  pub fn new(tx: Sender<String>, rx: Receiver<String>) -> InspectorHandle {
    InspectorHandle {
      tx: Arc::new(Mutex::new(tx)),
      rx: Arc::new(Mutex::new(rx)),
    }
  }
}

#[test]
fn test_v8_version() {
  assert!(v8_version().len() > 3);
}
