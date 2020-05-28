// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate downcast_rs;
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod any_error;
mod bindings;
mod core_isolate;
mod es_isolate;
mod flags;
mod js_errors;
mod module_specifier;
mod modules;
mod ops;
pub mod plugin_api;
mod resources;
mod shared_queue;
mod zero_copy_buf;

pub use rusty_v8 as v8;

pub use crate::any_error::*;
pub use crate::core_isolate::*;
pub use crate::es_isolate::*;
pub use crate::flags::v8_set_flags;
pub use crate::js_errors::*;
pub use crate::module_specifier::*;
pub use crate::modules::*;
pub use crate::ops::*;
pub use crate::resources::*;
pub use crate::zero_copy_buf::ZeroCopyBuf;

pub fn v8_version() -> &'static str {
  v8::V8::get_version()
}

#[test]
fn test_v8_version() {
  assert!(v8_version().len() > 3);
}

crate_modules!();
