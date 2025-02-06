// Copyright 2018-2025 the Deno authors. MIT license.

use std::env;

use deno_runtime::*;

fn main() {
  // Skip building from docs.rs.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }

  deno_napi::print_linker_flags("deno");
  deno_webgpu::print_linker_flags("deno");

  // Host snapshots won't work when cross compiling.
  let target = env::var("TARGET").unwrap();
  let host = env::var("HOST").unwrap();
  let skip_cross_check =
    env::var("DENO_SKIP_CROSS_BUILD_CHECK").map_or(false, |v| v == "1");
  if !skip_cross_check && target != host {
    panic!("Cross compiling with snapshot is not supported.");
  }

  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  if let Ok(c) = env::var("DENO_CANARY") {
    println!("cargo:rustc-env=DENO_CANARY={c}");
  }
  println!("cargo:rerun-if-env-changed=DENO_CANARY");

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());

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
