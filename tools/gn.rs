// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// This is used in cli/build.rs and core/build.rs to interface with the GN build
// system (which defines the deno build).

use std::collections::HashSet;
use std::env;
use std::path::{self, Path, PathBuf};
use std::process::Command;

pub struct Build {
  gn_mode: String,
  root: PathBuf,
  pub gn_out_dir: String,
  pub gn_out_path: PathBuf,
  pub check_only: bool,
}

impl Build {
  pub fn setup() -> Build {
    let gn_mode = if cfg!(target_os = "windows") {
      // On Windows, we need to link with a release build of libdeno, because
      // rust always uses the release CRT.
      // TODO(piscisaureus): make linking with debug libdeno possible.
      String::from("release")
    } else {
      // Cargo sets PROFILE to either "debug" or "release", which conveniently
      // matches the build modes we support.
      env::var("PROFILE").unwrap()
    };

    // cd into workspace root.
    assert!(env::set_current_dir("..").is_ok());

    let root = env::current_dir().unwrap();
    // If not using host default target the output folder will change
    // target/release will become target/$TARGET/release
    // Gn should also be using this output directory as well
    // most things will work with gn using the default
    // output directory but some tests depend on artifacts
    // being in a specific directory relative to the main build output
    let gn_out_path = root.join(format!("target/{}", gn_mode.clone()));
    let gn_out_dir = normalize_path(&gn_out_path);

    // Tell Cargo when to re-run this file. We do this first, so these directives
    // can take effect even if something goes wrong later in the build process.
    println!("cargo:rerun-if-env-changed=DENO_BUILD_PATH");
    // TODO: this is obviously not appropriate here.
    println!("cargo:rerun-if-env-changed=APPVEYOR_REPO_COMMIT");

    // This helps Rust source files locate the snapshot, source map etc.
    println!("cargo:rustc-env=GN_OUT_DIR={}", gn_out_dir);

    // Detect if we're being invoked by the rust language server (RLS).
    // Unfortunately we can't detect whether we're being run by `cargo check`.
    let check_only = env::var_os("CARGO")
      .map(PathBuf::from)
      .as_ref()
      .and_then(|p| p.file_stem())
      .and_then(|f| f.to_str())
      .map(|s| s.starts_with("rls"))
      .unwrap_or(false);

    if check_only {
      // Enable the 'check_only' feature, which enables some workarounds in the
      // rust source code to compile successfully without a bundle and snapshot
      println!("cargo:rustc-cfg=feature=\"check-only\"");
    }

    Build {
      gn_out_dir,
      gn_out_path,
      check_only,
      gn_mode,
      root,
    }
  }

  pub fn run(&self, gn_target: &str) {
    if !self.gn_out_path.join("build.ninja").exists() {
      let status = Command::new("python")
        .env("DENO_BUILD_PATH", &self.gn_out_dir)
        .env("DENO_BUILD_MODE", &self.gn_mode)
        .env("DEPOT_TOOLS_WIN_TOOLCHAIN", "0")
        .arg("./tools/setup.py")
        .status()
        .expect("setup.py failed");
      assert!(status.success());
    }

    rerun_if_changed(gn_target, &self.gn_out_path, &self.root);

    let mut ninja = ninja_command(&self.root);
    let ninja = command_env(&mut ninja, &self.root);

    let status = ninja
      .arg(gn_target)
      .arg("-C")
      .arg(&self.gn_out_dir)
      .status()
      .expect("ninja failed");
    assert!(status.success());
  }
}

fn command_env<'a>(cmd: &'a mut Command, root: &PathBuf) -> &'a mut Command {
  if !cfg!(target_os = "windows") {
    cmd
  } else {
    // Windows needs special configuration. This is similar to the function of
    // python_env() in //tools/util.py.
    let python_path: Vec<String> = vec![
      "third_party/python_packages",
      "third_party/python_packages/win32",
      "third_party/python_packages/win32/lib",
      "third_party/python_packages/Pythonwin",
    ].into_iter()
    .map(|p| root.join(p).into_os_string().into_string().unwrap())
    .collect();
    let orig_path =
      String::from(";") + &env::var_os("PATH").unwrap().into_string().unwrap();
    let path = root
      .join("third_party/python_packages/pywin32_system32")
      .into_os_string()
      .into_string()
      .unwrap();
    cmd
      .env("PYTHONPATH", python_path.join(";"))
      .env("PATH", path + &orig_path)
      .env("DEPOT_TOOLS_WIN_TOOLCHAIN", "0")
  }
}

// Utility function to make a path absolute, normalizing it to use forward
// slashes only. The returned value is an owned String, otherwise panics.
fn normalize_path<T: AsRef<Path>>(path: T) -> String {
  path
    .as_ref()
    .to_str()
    .unwrap()
    .to_owned()
    .chars()
    .map(|c| if path::is_separator(c) { '/' } else { c })
    .collect()
}

fn maybe_print_rerun_if_changed(dep: &str, gn_out_dir: &PathBuf) {
  let d = gn_out_dir.join(dep);
  println!(
    "cargo:rerun-if-changed={}",
    d.into_os_string().into_string().unwrap()
  );
}

fn gn_command(root: &PathBuf) -> Command {
  // Use DENO_GN_PATH if present.
  let p = env::var_os("DENO_GN_PATH").unwrap_or_else(|| {
    if cfg!(target_os = "windows") {
      root.join("third_party/depot_tools/gn.bat")
    } else {
      root.join("third_party/depot_tools/gn")
    }.into_os_string()
  });
  Command::new(p)
}

fn ninja_command(root: &PathBuf) -> Command {
  // Use DENO_NINJA_PATH if present.
  let p = env::var_os("DENO_NINJA_PATH").unwrap_or_else(|| {
    root.join("third_party/depot_tools/ninja").into_os_string()
  });
  Command::new(p)
}

fn rerun_if_changed(gn_target: &str, gn_out_dir: &PathBuf, root: &PathBuf) {
  // Notes on cargo:rerun-if-changed. The feature is very sensitive. If you do
  // not provide the path in the correct form, the build will always rebuild.
  // - Any invalid filename will cause rebuild.
  // - Do not put the path in quotes.
  // - Relative paths work when relative to the Cargo.toml directory.
  // - Absolute paths work (no quotes)

  // Example: gn desc target/debug/ cli:deno_deps runtime_deps
  let mut gn = gn_command(root);
  let output = command_env(&mut gn, root)
    .arg("desc")
    .arg(gn_out_dir)
    .arg(gn_target)
    .arg("runtime_deps")
    .output()
    .expect("gn desc failed");
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  for dep in stdout.lines() {
    maybe_print_rerun_if_changed(dep, gn_out_dir);
  }

  // Example: ninja -C target/debug/ cli:deno_deps -t deps
  let mut ninja = ninja_command(root);
  let output = command_env(&mut ninja, root)
    .arg("-C")
    .arg(gn_out_dir)
    .arg("-t")
    .arg("deps")
    .output()
    .expect("ninja -t deps failed");
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let deps = ninja_deps_parse(&stdout);
  for dep in deps.iter() {
    maybe_print_rerun_if_changed(dep, gn_out_dir);
  }
}

fn ninja_deps_parse(s: &str) -> HashSet<String> {
  let mut out = HashSet::new();
  for line in s.lines() {
    if line.starts_with("    ") {
      let filename = String::from(line.trim_start());
      out.insert(filename);
    }
  }
  out
}

#[test]
fn test_parse_ninja_deps_output() {
  const NINJA_DEPS_OUTPUT: &'static str = r#"
obj/foo/foo.o: #deps 2, deps mtime 1562002734 (VALID)
    ../../../../../../example/src/foo.cc
    ../../../../../../example/src/hello.h

obj/hello/hello.o: #deps 2, deps mtime 1562002734 (VALID)
    ../../../../../../example/src/hello.cc
    ../../../../../../example/src/hello.h
  "#;

  let foo_cc = "../../../../../../example/src/foo.cc".to_string();
  let hello_cc = "../../../../../../example/src/hello.cc".to_string();
  let hello_h = "../../../../../../example/src/hello.h".to_string();
  let blah_h = "../../../../../../example/src/blah.h".to_string();

  let set = ninja_deps_parse(NINJA_DEPS_OUTPUT);

  assert!(set.contains(&foo_cc));
  assert!(set.contains(&hello_cc));
  assert!(set.contains(&hello_h));
  assert!(!set.contains(&blah_h));
}
