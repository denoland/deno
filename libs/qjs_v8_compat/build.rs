// Copyright 2018-2026 the Deno authors. MIT license.
//
// build.rs for qjs_v8_compat.
//
// The default build is a no-op: the safe wrapper compiles against extern FFI
// declarations without linking any C library. This lets the type surface be
// validated by `cargo check` even on machines without QuickJS-ng installed.
//
// With `--features link_quickjs`, the build script compiles the vendored
// QuickJS-ng submodule (`vendor/quickjs-ng/`) via the `cc` crate and links
// it as a static library. The four core sources (quickjs.c, libregexp.c,
// libunicode.c, dtoa.c) match the upstream CMakeLists `qjs_sources` set.
//
// Override the vendored build by setting any of:
//   QUICKJS_NG_LIB_DIR    — explicit directory containing libquickjs.{a,so}
//   QUICKJS_NG_DIR        — built QuickJS-ng tree (looks in itself, build/,
//                           build/Release/ for the library)
//   QUICKJS_NG_STATIC=1   — when using LIB_DIR/DIR, force static linkage

use std::env;
use std::path::Path;
use std::path::PathBuf;

fn main() {
  println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-env-changed=QUICKJS_NG_DIR");
  println!("cargo:rerun-if-env-changed=QUICKJS_NG_LIB_DIR");
  println!("cargo:rerun-if-env-changed=QUICKJS_NG_STATIC");

  let link_quickjs = env::var_os("CARGO_FEATURE_LINK_QUICKJS").is_some();
  if !link_quickjs {
    return;
  }

  // Honor explicit overrides first — distros/CI may have a prebuilt tree.
  if let Some(dir) = env::var_os("QUICKJS_NG_LIB_DIR") {
    let p = PathBuf::from(dir);
    println!("cargo:rustc-link-search=native={}", p.display());
    println!("cargo:rustc-link-lib={}=quickjs", external_link_kind());
    return;
  }
  if let Some(dir) = env::var_os("QUICKJS_NG_DIR") {
    let p = PathBuf::from(dir);
    for sub in ["", "build", "build/Release"] {
      let cand = p.join(sub);
      if cand.exists() {
        println!("cargo:rustc-link-search=native={}", cand.display());
      }
    }
    println!("cargo:rustc-link-lib={}=quickjs", external_link_kind());
    return;
  }

  build_vendored();
}

fn external_link_kind() -> &'static str {
  let static_link = matches!(
    env::var("QUICKJS_NG_STATIC").as_deref(),
    Ok("1") | Ok("true") | Ok("yes")
  );
  if static_link { "static" } else { "dylib" }
}

// Compile the vendored QuickJS-ng sources via the `cc` crate.
//
// The four files below mirror upstream CMakeLists.txt's `qjs_sources`. The
// `_GNU_SOURCE` define matches `qjs_defines` from the same file. Warnings are
// disabled because QuickJS-ng's C is compiled with -Wno-* for several legacy
// constructs and we don't want spurious build failures from clippy CI's
// strict warning-as-error settings on the C compiler.
fn build_vendored() {
  let manifest_dir =
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
  let vendor = manifest_dir.join("vendor").join("quickjs-ng");

  let main_src = vendor.join("quickjs.c");
  if !main_src.exists() {
    // Surface as a `cargo:warning` + an emitted error rather than panicking
    // so the message reaches CI logs cleanly (panic in build.rs prints a
    // less-useful backtrace). The user almost certainly forgot to init the
    // submodule. Override paths still let them build against a prebuilt
    // tree without the submodule.
    println!(
      "cargo:warning=qjs_v8_compat: vendored QuickJS-ng not found at {}",
      vendor.display()
    );
    println!(
      "cargo:warning=qjs_v8_compat: run `git submodule update --init libs/qjs_v8_compat/vendor/quickjs-ng` to fetch it"
    );
    println!(
      "cargo:warning=qjs_v8_compat: or set QUICKJS_NG_LIB_DIR / QUICKJS_NG_DIR to point to a prebuilt tree"
    );
    panic!(
      "qjs_v8_compat: vendored QuickJS-ng submodule missing at {}",
      vendor.display()
    );
  }

  println!("cargo:rerun-if-changed={}", vendor.display());

  let mut build = cc::Build::new();
  build
    .file(vendor.join("quickjs.c"))
    .file(vendor.join("libregexp.c"))
    .file(vendor.join("libunicode.c"))
    .file(vendor.join("dtoa.c"))
    .include(&vendor)
    .define("_GNU_SOURCE", None)
    .warnings(false)
    .extra_warnings(false);

  if cfg!(target_os = "windows") {
    build.define("WIN32_LEAN_AND_MEAN", None);
    build.define("_WIN32_WINNT", "0x0601");
  }

  // QuickJS allocates more stack than the cargo-default in some recursive
  // paths; opt into the default optimisation level even in debug, otherwise
  // bytecode compilation is unusably slow under cargo test.
  if env::var("PROFILE").as_deref() == Ok("debug") {
    build.opt_level(2);
  }

  build.compile("quickjs");

  // Link math + dl + pthread (matches `qjs_libs` in CMakeLists.txt).
  println!("cargo:rustc-link-lib=m");
  if !cfg!(target_os = "windows") {
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=pthread");
  }

  let _ = Path::new(""); // silence unused-import warning when target gating excludes a path
}
