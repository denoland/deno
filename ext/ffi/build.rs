// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::env;

fn build_tcc() {
  {
    // TODO(@littledivy): Windows support for fast call.
    // let tcc_path = root
    //   .parent()
    //   .unwrap()
    //   .to_path_buf()
    //   .parent()
    //   .unwrap()
    //   .to_path_buf()
    //   .join("third_party")
    //   .join("prebuilt")
    //   .join("win");
    // println!("cargo:rustc-link-search=native={}", tcc_path.display());
  }
  #[cfg(not(target_os = "windows"))]
  {
    use std::path::PathBuf;
    use std::process::exit;
    use std::process::Command;

    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR")));
    let tcc_src = root.join("tinycc");
    dbg!(&tcc_src);
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
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
}

#[cfg(target_os = "windows")]
fn main() {}

#[cfg(not(target_os = "windows"))]
fn main() {
  if let Ok(tcc_path) = env::var("TCC_PATH") {
    println!("cargo:rustc-link-search=native={}", tcc_path);
  } else {
    build_tcc();
  }
  println!("cargo:rustc-link-lib=static=tcc");
}
