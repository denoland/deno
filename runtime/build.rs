// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod shared;

use std::env;
use std::path::PathBuf;

#[cfg(all(
  not(feature = "docsrs"),
  not(feature = "dont_create_runtime_snapshot")
))]
mod snapshot;

fn main() {
  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let runtime_snapshot_path = o.join("RUNTIME_SNAPSHOT.bin");

  // If we're building on docs.rs we just create
  // and empty snapshot file and return, because `rusty_v8`
  // doesn't actually compile on docs.rs
  if env::var_os("DOCS_RS").is_some() {
    let snapshot_slice = &[];
    #[allow(clippy::needless_borrow)]
    #[allow(clippy::disallowed_methods)]
    std::fs::write(&runtime_snapshot_path, snapshot_slice).unwrap();
  }

  #[cfg(all(
    not(feature = "docsrs"),
    not(feature = "dont_create_runtime_snapshot")
  ))]
  snapshot::create_runtime_snapshot(runtime_snapshot_path)
}
