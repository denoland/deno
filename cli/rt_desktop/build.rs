// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(
  clippy::disallowed_macros,
  reason = "build script directives must go to stdout"
)]
#![allow(clippy::disallowed_methods, reason = "build code")]

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
  {
    println!(
      "cargo:rustc-cdylib-link-arg=-Wl,--export-dynamic-symbol-list={}",
      symbols_path.display(),
    );

    // V8 (via rusty_v8's `use_custom_libcxx`) statically links its own
    // libc++/libc++abi into this cdylib. A desktop backend (laufey) `dlopen`s
    // us into a process that already uses the system libstdc++. Without hiding
    // our bundled C++ runtime symbols, ELF interposition binds our internal
    // `__cxa_guard_acquire` (and friends) to the host's libstdc++, whose guard
    // layout differs — the runtime aborts at static init with
    // "libc++abi: __cxa_guard_acquire failed to acquire mutex" before
    // `Deno.serve` ever runs (denoland/deno#35381).
    //
    // `--exclude-libs,ALL` localizes every static-archive symbol (including the
    // bundled libc++abi) so our calls resolve to our own copy. The NAPI/uv
    // exports above are re-added explicitly, and the `laufey_runtime_*` entry
    // points live in this crate's own objects (not an archive), so both stay
    // visible.
    println!("cargo:rustc-cdylib-link-arg=-Wl,--exclude-libs,ALL");
  }
}
