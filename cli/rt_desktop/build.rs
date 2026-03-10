// Copyright 2018-2026 the Deno authors. MIT license.

fn main() {
  // Skip building from docs.rs.
  if std::env::var_os("DOCS_RS").is_some() {
    return;
  }

  // For cdylib targets, we must explicitly export NAPI symbols so that
  // native addons (e.g. next-swc) can resolve them via dlsym.
  // The print_linker_flags() functions use cargo:rustc-link-arg-bin
  // which only applies to binary targets, so we emit cdylib-specific
  // linker args here instead.
  print_cdylib_napi_linker_flags();
}

fn print_cdylib_napi_linker_flags() {
  let symbols_file_name = match std::env::consts::OS {
    "android" | "freebsd" | "openbsd" => {
      "generated_symbol_exports_list_linux.def".to_string()
    }
    os => format!("generated_symbol_exports_list_{}.def", os),
  };

  // Path relative to this build script's Cargo.toml
  let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let symbols_path = std::path::Path::new(&manifest_dir)
    .join("../../ext/napi")
    .join(&symbols_file_name)
    .canonicalize()
    .expect("Missing NAPI symbols list");

  println!("cargo:rerun-if-changed={}", symbols_path.display());

  #[cfg(target_os = "macos")]
  println!(
    "cargo:rustc-cdylib-link-arg=-Wl,-exported_symbols_list,{}",
    symbols_path.display(),
  );

  #[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  println!(
    "cargo:rustc-cdylib-link-arg=-Wl,--export-dynamic-symbol-list={}",
    symbols_path.display(),
  );
}
