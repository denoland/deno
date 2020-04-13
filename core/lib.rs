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
mod es_isolate;
mod flags;
mod isolate;
mod js_errors;
mod module_specifier;
mod modules;
mod ops;
mod plugins;
mod resources;
mod shared_queue;

pub use rusty_v8 as v8;

pub use crate::any_error::*;
pub use crate::es_isolate::*;
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

crate_modules!();
