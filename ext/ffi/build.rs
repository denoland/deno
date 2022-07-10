// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::PathBuf;
use std::process::exit;
use std::process::Command;

fn build_tcc() {
  let tcc_src = env::current_dir().unwrap().join("tinycc");
  let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
  println!("tcc_src: {:?}", tcc_src);
  let mut configure = Command::new(tcc_src.join("configure"));
  configure.current_dir(&out_dir);
  configure.args(&["--enable-static", "--extra-cflags=-fPIC -O3 -g -static"]);
  let status = configure.status().unwrap();
  if !status.success() {
    eprintln!("Fail to configure: {:?}", status);
    exit(1);
  }

  let mut make = Command::new("make");
  make.current_dir(&out_dir).arg(format!(
    "-j{}",
    env::var("NUM_JOBS").unwrap_or_else(|_| String::from("1"))
  ));
  make.args(&["libtcc.a"]);
  let status = make.status().unwrap();

  if !status.success() {
    eprintln!("Fail to make: {:?}", status);
    exit(1);
  }

  println!("cargo:rustc-link-search=native={}", out_dir.display());
  println!("cargo:rerun-if-changed={}", tcc_src.display());
}

fn main() {
  build_tcc();
  let target = env::var("TARGET").unwrap();
  if target.contains("msvc") {
    println!("cargo:rustc-link-lib=static=libtcc");
  } else {
    println!("cargo:rustc-link-lib=static=tcc");
  }
}
