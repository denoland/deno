// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! A replacement for https://crates.io/crates/dirs which as of 2.0.2 requires a
//! chain of crate dependencies dirs, dirs-next, dirs-sys, dirs-sys-next all to
//! supply a very basic function. It's not worth the linking time.

use std::path::PathBuf;

pub fn cache_dir() -> Option<PathBuf> {
  todo!()
}

pub fn home_dir() -> Option<PathBuf> {
  todo!()
}
