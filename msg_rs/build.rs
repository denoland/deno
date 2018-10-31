// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// This file doesn't actually attempt to to build anything. It just tries to
// guess where the generated flatbuffers source file 'msg_generated.rs' lives.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
  let build_dir = match env::var("DENO_BUILD_PATH") {
    Ok(s) => {
      // If DENO_BUILD_PATH is set, msg_generated.rs can be found here.
      Path::new(&s).to_path_buf()
    }
    Err(_) => {
      // If DENO_BUILD_PATH is not set, derive the build dir from the source
      // directory and the cargo build mode. Note that cargo sets $PROFILE
      // to either "debug" or "release", so that's quite convenient.
      let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
      let crate_dir = Path::new(&crate_dir);
      let src_root_dir = crate_dir.parent().unwrap();
      let profile = env::var("PROFILE").unwrap();
      src_root_dir.join("out").join(profile)
    }
  };

  let msg_gen_rs = build_dir.join("gen/msg_generated.rs");
  let msg_gen_rs = msg_gen_rs.to_string_lossy().replace("\\", "/");

  let out_dir = env::var("OUT_DIR").unwrap();
  let out_dir = Path::new(&out_dir);
  let wrapper_rs = out_dir.join("find_msg_generated.inc.rs");

  let mut wrapper_file = File::create(&wrapper_rs).unwrap();
  write!(
    wrapper_file,
    "
      #[path = \"{}\"]
      mod msg;
      pub use msg::*;
    ",
    msg_gen_rs
  ).unwrap();

  println!("cargo:rerun-if-env-changed=DENO_BUILD_PATH");
}
