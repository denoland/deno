// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::json_op_sync;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;

fn create_snapshot(
  mut js_runtime: JsRuntime,
  snapshot_path: &Path,
  files: Vec<PathBuf>,
) {
  deno_web::init(&mut js_runtime);
  deno_fetch::init(&mut js_runtime);
  deno_crypto::init(&mut js_runtime);
  deno_webgpu::init(&mut js_runtime);
  // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the
  // workspace root.
  let display_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
  for file in files {
    println!("cargo:rerun-if-changed={}", file.display());
    let display_path = file.strip_prefix(display_root).unwrap();
    let display_path_str = display_path.display().to_string();
    js_runtime
      .execute(
        &("deno:".to_string() + &display_path_str.replace('\\', "/")),
        &std::fs::read_to_string(&file).unwrap(),
      )
      .unwrap();
  }

  let snapshot = js_runtime.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  std::fs::write(&snapshot_path, snapshot_slice).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());
}

fn create_runtime_snapshot(snapshot_path: &Path, files: Vec<PathBuf>) {
  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    ..Default::default()
  });
  create_snapshot(js_runtime, snapshot_path, files);
}

#[derive(Debug, Deserialize)]
struct LoadArgs {
  /// The fully qualified specifier that should be loaded.
  specifier: String,
}

fn create_compiler_snapshot(
  snapshot_path: &Path,
  files: Vec<PathBuf>,
  cwd: &Path,
) {
  // libs that are being provided by op crates.
  let mut op_crate_libs = HashMap::new();
  op_crate_libs.insert("deno.web", deno_web::get_declaration());
  op_crate_libs.insert("deno.fetch", deno_fetch::get_declaration());

  // ensure we invalidate the build properly.
  for (_, path) in op_crate_libs.iter() {
    println!("cargo:rerun-if-changed={}", path.display());
  }

  // libs that should be loaded into the isolate before snapshotting.
  let libs = vec![
    // Deno custom type libraries
    "deno.window",
    "deno.worker",
    "deno.shared_globals",
    "deno.ns",
    "deno.unstable",
    // Deno built-in type libraries
    "es5",
    "es2015.collection",
    "es2015.core",
    "es2015",
    "es2015.generator",
    "es2015.iterable",
    "es2015.promise",
    "es2015.proxy",
    "es2015.reflect",
    "es2015.symbol",
    "es2015.symbol.wellknown",
    "es2016.array.include",
    "es2016",
    "es2017",
    "es2017.intl",
    "es2017.object",
    "es2017.sharedmemory",
    "es2017.string",
    "es2017.typedarrays",
    "es2018.asyncgenerator",
    "es2018.asynciterable",
    "es2018",
    "es2018.intl",
    "es2018.promise",
    "es2018.regexp",
    "es2019.array",
    "es2019",
    "es2019.object",
    "es2019.string",
    "es2019.symbol",
    "es2020.bigint",
    "es2020",
    "es2020.intl",
    "es2020.promise",
    "es2020.sharedmemory",
    "es2020.string",
    "es2020.symbol.wellknown",
    "esnext",
    "esnext.intl",
    "esnext.promise",
    "esnext.string",
    "esnext.weakref",
  ];

  // create a copy of the vector that includes any op crate libs to be passed
  // to the JavaScript compiler to build into the snapshot
  let mut build_libs = libs.clone();
  for (op_lib, _) in op_crate_libs.iter() {
    build_libs.push(op_lib.to_owned());
  }

  let re_asset = Regex::new(r"asset:/{3}lib\.(\S+)\.d\.ts").expect("bad regex");
  let path_dts = cwd.join("dts");
  let build_specifier = "asset:///bootstrap.ts";

  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    ..Default::default()
  });
  js_runtime.register_op(
    "op_build_info",
    json_op_sync(move |_state, _args, _bufs| {
      Ok(json!({
        "buildSpecifier": build_specifier,
        "libs": build_libs,
      }))
    }),
  );
  // using the same op that is used in `tsc.rs` for loading modules and reading
  // files, but a slightly different implementation at build time.
  js_runtime.register_op(
    "op_load",
    json_op_sync(move |_state, args, _bufs| {
      let v: LoadArgs = serde_json::from_value(args)?;
      // we need a basic file to send to tsc to warm it up.
      if v.specifier == build_specifier {
        Ok(json!({
          "data": r#"console.log("hello deno!");"#,
          "hash": "1",
          // this corresponds to `ts.ScriptKind.TypeScript`
          "scriptKind": 3
        }))
      // specifiers come across as `asset:///lib.{lib_name}.d.ts` and we need to
      // parse out just the name so we can lookup the asset.
      } else if let Some(caps) = re_asset.captures(&v.specifier) {
        if let Some(lib) = caps.get(1).map(|m| m.as_str()) {
          // if it comes from an op crate, we were supplied with the path to the
          // file.
          let path = if let Some(op_crate_lib) = op_crate_libs.get(lib) {
            op_crate_lib.clone()
          // otherwise we are will generate the path ourself
          } else {
            path_dts.join(format!("lib.{}.d.ts", lib))
          };
          let data = std::fs::read_to_string(path)?;
          Ok(json!({
            "data": data,
            "hash": "1",
            // this corresponds to `ts.ScriptKind.TypeScript`
            "scriptKind": 3
          }))
        } else {
          Err(custom_error(
            "InvalidSpecifier",
            format!("An invalid specifier was requested: {}", v.specifier),
          ))
        }
      } else {
        Err(custom_error(
          "InvalidSpecifier",
          format!("An invalid specifier was requested: {}", v.specifier),
        ))
      }
    }),
  );
  create_snapshot(js_runtime, snapshot_path, files);
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

fn git_commit_hash() -> String {
  if let Ok(output) = std::process::Command::new("git")
    .arg("rev-list")
    .arg("-1")
    .arg("HEAD")
    .output()
  {
    if output.status.success() {
      std::str::from_utf8(&output.stdout[..7])
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

fn main() {
  // Don't build V8 if "cargo doc" is being run. This is to support docs.rs.
  if env::var_os("RUSTDOCFLAGS").is_some() {
    return;
  }

  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  println!("cargo:rustc-env=TS_VERSION={}", ts_version());
  println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_commit_hash());
  println!(
    "cargo:rustc-env=DENO_WEB_LIB_PATH={}",
    deno_web::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_FETCH_LIB_PATH={}",
    deno_fetch::get_declaration().display()
  );

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
  if let Ok(c) = env::var("DENO_CANARY") {
    println!("cargo:rustc-env=DENO_CANARY={}", c);
  }

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let runtime_snapshot_path = o.join("CLI_SNAPSHOT.bin");
  let compiler_snapshot_path = o.join("COMPILER_SNAPSHOT.bin");

  let js_files = get_js_files("rt");
  create_runtime_snapshot(&runtime_snapshot_path, js_files);

  let js_files = get_js_files("tsc");
  create_compiler_snapshot(&compiler_snapshot_path, js_files, &c);

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

fn get_js_files(d: &str) -> Vec<PathBuf> {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut js_files = std::fs::read_dir(d)
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      manifest_dir.join(file.path())
    })
    .filter(|path| path.extension().unwrap_or_default() == "js")
    .collect::<Vec<PathBuf>>();
  js_files.sort();
  js_files
}
