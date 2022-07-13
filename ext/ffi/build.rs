// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::env;

#[cfg(not(target_os = "windows"))]
fn static_lib_path() -> Option<String> {
  env::var("DENO_FFI_LIBTCC").ok()
}

fn setup_tcc() {
  #[cfg(not(target_os = "windows"))]
  {
    let lib_path = static_lib_path();

    match lib_path {
      Some(path) => {
        println!("static lib path: {}", path);
        cargo_conf(&path, &path);
      }
      None => {
        let (lib_path, src) = build_tcc();
        cargo_conf(&lib_path, &src);
      }
    }
  }
}

#[cfg(not(target_os = "windows"))]
fn cargo_conf(lib_path: &String, update_path: &String) {
  println!("cargo:rustc-link-search=native={}", lib_path);
  println!("cargo:rerun-if-changed={}", update_path);
}

fn build_tcc() -> (String, String) {
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
    (out_dir.display().to_string(), tcc_src.display().to_string())
  }
}

#[cfg(target_os = "windows")]
fn main() {}

#[cfg(not(target_os = "windows"))]
fn main() {
  setup_tcc();
  println!("cargo:rustc-link-lib=static=tcc");
}
