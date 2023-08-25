// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
  println!("cargo:rerun-if-changed=./proto");

  let descriptor_path =
    PathBuf::from(env::var("OUT_DIR").unwrap()).join("proto_descriptor.bin");

  prost_build::Config::new()
    .file_descriptor_set_path(&descriptor_path)
    .compile_well_known_types()
    .compile_protos(&["proto/datapath.proto"], &["proto/"])?;

  Ok(())
}
