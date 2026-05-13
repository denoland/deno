// Copyright 2018-2026 the Deno authors. MIT license.
//
// build.rs for qjs_v8_compat.
//
// The default build is a no-op: the safe wrapper compiles against extern FFI
// declarations without linking any C library. This lets the type surface be
// validated by `cargo check` even on machines without QuickJS-ng installed.
//
// With `--features link_quickjs`, we link against QuickJS-ng. The library
// search path is discovered (in order) from:
//   QUICKJS_NG_LIB_DIR  (explicit directory containing libquickjs.{a,so})
//   QUICKJS_NG_DIR      (a QuickJS-ng source/build tree)
//   pkg-config (quickjs-ng package)
//   /usr/local/lib, /usr/lib (last-resort common paths)
//
// We deliberately do not vendor QuickJS-ng source here: it adds ~50K LOC of C
// to the deno_core tree and forces a CMake or cc-crate build for every contrib
// who doesn't care about this backend. Distros and CI jobs that want the
// QuickJS backend point QUICKJS_NG_DIR at a prebuilt tree.

use std::env;
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

  let static_link = matches!(
    env::var("QUICKJS_NG_STATIC").as_deref(),
    Ok("1") | Ok("true") | Ok("yes")
  );
  let link_kind = if static_link { "static" } else { "dylib" };

  if let Some(dir) = env::var_os("QUICKJS_NG_LIB_DIR") {
    let p = PathBuf::from(dir);
    println!("cargo:rustc-link-search=native={}", p.display());
    println!("cargo:rustc-link-lib={}=quickjs", link_kind);
    return;
  }

  if let Some(dir) = env::var_os("QUICKJS_NG_DIR") {
    let p = PathBuf::from(dir);
    // QuickJS-ng's CMake build emits libquickjs.{a,so} into the build dir.
    for sub in ["", "build", "build/Release"] {
      let cand = p.join(sub);
      if cand.exists() {
        println!("cargo:rustc-link-search=native={}", cand.display());
      }
    }
    println!("cargo:rustc-link-lib={}=quickjs", link_kind);
    return;
  }

  // Last resort: assume libquickjs is on the default link search path.
  println!("cargo:rustc-link-lib={}=quickjs", link_kind);
  println!(
    "cargo:warning=qjs_v8_compat: linking against quickjs from default \
     search path; set QUICKJS_NG_DIR or QUICKJS_NG_LIB_DIR to override"
  );
}
