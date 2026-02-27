// Copyright 2018-2026 the Deno authors. MIT license.

extern crate napi_build;

fn main() {
  napi_build::setup();

  // The bundled node.lib from napi-build may not include newer experimental
  // NAPI symbols. On Windows, generate a supplementary import library so the
  // DLL can link against them.
  #[cfg(target_os = "windows")]
  {
    let out_dir =
      std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let def_path = out_dir.join("extra_napi.def");
    std::fs::write(
      &def_path,
      "LIBRARY\nEXPORTS\n  node_api_create_object_with_properties\n",
    )
    .unwrap();

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let machine = match arch.as_str() {
      "x86_64" => "X64",
      "x86" => "X86",
      "aarch64" => "ARM64",
      other => panic!("Unsupported architecture: {other}"),
    };

    let lib_path = out_dir.join("extra_napi.lib");
    let status = std::process::Command::new("lib.exe")
      .arg(format!("/DEF:{}", def_path.display()))
      .arg(format!("/OUT:{}", lib_path.display()))
      .arg(format!("/MACHINE:{machine}"))
      .status()
      .expect("failed to run lib.exe");
    assert!(status.success(), "lib.exe failed");

    println!("cargo:rustc-link-lib=extra_napi");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
  }
}
