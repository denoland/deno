// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![deny(warnings)]

#[macro_use]
extern crate log;
extern crate futures;
extern crate libc;
#[macro_use]
extern crate downcast_rs;
extern crate rusty_v8;
#[macro_use]
extern crate lazy_static;

mod any_error;
mod bindings;
mod flags;
mod isolate;
mod js_errors;
mod module_specifier;
mod modules;
mod ops;
mod plugins;
mod resources;
mod shared_queue;

use rusty_v8 as v8;

pub use crate::any_error::*;
pub use crate::flags::v8_set_flags;
pub use crate::isolate::*;
pub use crate::js_errors::*;
pub use crate::module_specifier::*;
pub use crate::modules::*;
pub use crate::ops::*;
pub use crate::plugins::*;
pub use crate::resources::*;

pub fn v8_version() -> &'static str {
  v8::V8::get_version()
}

#[test]
fn test_v8_version() {
  assert!(v8_version().len() > 3);
}
