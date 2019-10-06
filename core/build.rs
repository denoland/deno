// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run "cargo build -vv" if you want to see gn output.

fn main() {
  let build = gn::Build::setup();

  println!("cargo:rustc-link-search=native={}/obj", build.gn_out_dir);

  build.run("default");
}

mod gn {
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
        let mut cmd = Command::new("python");
        cmd.env("DENO_BUILD_PATH", &self.gn_out_dir);
        cmd.env("DENO_BUILD_MODE", &self.gn_mode);
        cmd.env("DEPOT_TOOLS_WIN_TOOLCHAIN", "0");
        cmd.arg("./tools/setup.py");
        if env::var_os("DENO_NO_BINARY_DOWNLOAD").is_some() {
          cmd.arg("--no-binary-download");
        }
        let status = cmd.status().expect("setup.py failed");
        assert!(status.success());
      }

      let mut ninja = Command::new("third_party/depot_tools/ninja");
      let ninja = if !cfg!(target_os = "windows") {
        &mut ninja
      } else {
        // Windows needs special configuration. This is similar to the function of
        // python_env() in //tools/util.py.
        let python_path: Vec<String> = vec![
          "third_party/python_packages",
          "third_party/python_packages/win32",
          "third_party/python_packages/win32/lib",
          "third_party/python_packages/Pythonwin",
        ]
        .into_iter()
        .map(|p| self.root.join(p).into_os_string().into_string().unwrap())
        .collect();
        let orig_path = String::from(";")
          + &env::var_os("PATH").unwrap().into_string().unwrap();
        let path = self
          .root
          .join("third_party/python_packages/pywin32_system32")
          .into_os_string()
          .into_string()
          .unwrap();
        ninja
          .env("PYTHONPATH", python_path.join(";"))
          .env("PATH", path + &orig_path)
          .env("DEPOT_TOOLS_WIN_TOOLCHAIN", "0")
      };

      let status = ninja
        .arg(gn_target)
        .arg("-C")
        .arg(&self.gn_out_dir)
        .status()
        .expect("ninja failed");
      assert!(status.success());
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
}
