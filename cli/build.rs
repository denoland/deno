// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_broadcast_channel;
use deno_runtime::deno_console;
use deno_runtime::deno_crypto;
use deno_runtime::deno_fetch;
use deno_runtime::deno_net;
use deno_runtime::deno_url;
use deno_runtime::deno_web;
use deno_runtime::deno_websocket;
use deno_runtime::deno_webstorage;

use std::env;
use std::path::Path;
use std::path::PathBuf;

fn main() {
  // Skip building from docs.rs.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }

  // Host snapshots won't work when cross compiling.
  let target = env::var("TARGET").unwrap();
  let host = env::var("HOST").unwrap();
  if target != host {
    panic!("Cross compiling with snapshot is not supported.");
  }

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());

  if let Ok(c) = env::var("DENO_CANARY") {
    println!("cargo:rustc-env=DENO_CANARY={}", c);
  }
  println!("cargo:rerun-if-env-changed=DENO_CANARY");

  println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_commit_hash());
  println!("cargo:rerun-if-env-changed=GIT_COMMIT_HASH");

  println!("cargo:rustc-env=TS_VERSION={}", ts_version());
  println!("cargo:rerun-if-env-changed=TS_VERSION");

  println!(
    "cargo:rustc-env=DENO_CONSOLE_LIB_PATH={}",
    deno_console::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_URL_LIB_PATH={}",
    deno_url::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEB_LIB_PATH={}",
    deno_web::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_FETCH_LIB_PATH={}",
    deno_fetch::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBGPU_LIB_PATH={}",
    deno_webgpu_get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBSOCKET_LIB_PATH={}",
    deno_websocket::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBSTORAGE_LIB_PATH={}",
    deno_webstorage::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_CRYPTO_LIB_PATH={}",
    deno_crypto::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_BROADCAST_CHANNEL_LIB_PATH={}",
    deno_broadcast_channel::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_NET_LIB_PATH={}",
    deno_net::get_declaration().display()
  );

  #[cfg(target_os = "windows")]
  {
    let mut res = winres::WindowsResource::new();
    res.set_icon("deno.ico");
    res.set_language(winapi::um::winnt::MAKELANGID(
      winapi::um::winnt::LANG_ENGLISH,
      winapi::um::winnt::SUBLANG_ENGLISH_US,
    ));
    res.compile().unwrap();
  }
}

fn deno_webgpu_get_declaration() -> PathBuf {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  manifest_dir.join("dts").join("lib.deno_webgpu.d.ts")
}

fn git_commit_hash() -> String {
  if let Ok(output) = std::process::Command::new("git")
    .arg("rev-list")
    .arg("-1")
    .arg("HEAD")
    .output()
  {
    if output.status.success() {
      std::str::from_utf8(&output.stdout[..40])
        .unwrap()
        .to_string()
    } else {
      // When not in git repository
      // (e.g. when the user install by `cargo install deno`)
      "UNKNOWN".to_string()
    }
  } else {
    // When there is no git command for some reason
    "UNKNOWN".to_string()
  }
}

fn ts_version() -> String {
  std::fs::read_to_string("tsc/00_typescript.js")
    .unwrap()
    .lines()
    .find(|l| l.contains("ts.version = "))
    .expect(
      "Failed to find the pattern `ts.version = ` in typescript source code",
    )
    .chars()
    .skip_while(|c| !char::is_numeric(*c))
    .take_while(|c| *c != '"')
    .collect::<String>()
}
