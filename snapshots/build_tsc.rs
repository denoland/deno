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

use deno_runtime::deno_core::error::custom_error;
use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_core::op;
use deno_runtime::deno_core::serde::Deserialize;
use deno_runtime::deno_core::serde_json::json;
use deno_runtime::deno_core::serde_json::Value;
use deno_runtime::deno_core::Extension;
use deno_runtime::deno_core::JsRuntime;
use deno_runtime::deno_core::OpState;
use deno_runtime::deno_core::RuntimeOptions;

use regex::Regex;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::path::Path;
use std::path::PathBuf;

pub fn create_tsc_snapshot(snapshot_path: &Path) {
  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    extensions: vec![tsc_snapshot_init()],
    ..Default::default()
  });
  load_js_files(&mut js_runtime);
  write_snapshot(js_runtime, snapshot_path);
}

// TODO(bartlomieju): this module contains a lot of duplicated
// logic with `build_runtime.rs`
fn write_snapshot(mut js_runtime: JsRuntime, snapshot_path: &Path) {
  let snapshot = js_runtime.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());

  let compressed_snapshot_with_size = {
    let mut vec = vec![];

    vec.extend_from_slice(
      &u32::try_from(snapshot.len())
        .expect("snapshot larger than 4gb")
        .to_le_bytes(),
    );

    vec.extend_from_slice(
      &zstd::bulk::compress(snapshot_slice, 22)
        .expect("snapshot compression failed"),
    );

    vec
  };

  println!(
    "Snapshot compressed size: {}",
    compressed_snapshot_with_size.len()
  );

  std::fs::write(&snapshot_path, compressed_snapshot_with_size).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());
}

#[derive(Debug, Deserialize)]
struct LoadArgs {
  /// The fully qualified specifier that should be loaded.
  specifier: String,
}

fn tsc_snapshot_init() -> Extension {
  // libs that are being provided by op crates.
  let mut op_crate_libs = HashMap::new();
  op_crate_libs.insert("deno.console", deno_console::get_declaration());
  op_crate_libs.insert("deno.url", deno_url::get_declaration());
  op_crate_libs.insert("deno.web", deno_web::get_declaration());
  op_crate_libs.insert("deno.fetch", deno_fetch::get_declaration());
  op_crate_libs.insert("deno.webgpu", deno_webgpu_get_declaration());
  op_crate_libs.insert("deno.websocket", deno_websocket::get_declaration());
  op_crate_libs.insert("deno.webstorage", deno_webstorage::get_declaration());
  op_crate_libs.insert("deno.crypto", deno_crypto::get_declaration());
  op_crate_libs.insert(
    "deno.broadcast_channel",
    deno_broadcast_channel::get_declaration(),
  );
  op_crate_libs.insert("deno.net", deno_net::get_declaration());

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
    "es2020.date",
    "es2020.intl",
    "es2020.number",
    "es2020.promise",
    "es2020.sharedmemory",
    "es2020.string",
    "es2020.symbol.wellknown",
    "es2021",
    "es2021.intl",
    "es2021.promise",
    "es2021.string",
    "es2021.weakref",
    "es2022",
    "es2022.array",
    "es2022.error",
    "es2022.intl",
    "es2022.object",
    "es2022.string",
    "esnext",
    "esnext.array",
    "esnext.intl",
  ];

  let cli_dir = cli_dir();
  let path_dts = cli_dir.join("dts");
  // ensure we invalidate the build properly.
  for name in libs.iter() {
    println!(
      "cargo:rerun-if-changed={}",
      path_dts.join(format!("lib.{}.d.ts", name)).display()
    );
  }

  // create a copy of the vector that includes any op crate libs to be passed
  // to the JavaScript compiler to build into the snapshot
  let mut build_libs = libs.clone();
  for (op_lib, _) in op_crate_libs.iter() {
    build_libs.push(op_lib.to_owned());
  }

  #[op]
  fn op_build_info(state: &mut OpState) -> Value {
    let build_specifier = "asset:///bootstrap.ts";
    let build_libs = state.borrow::<Vec<&str>>();
    json!({
      "buildSpecifier": build_specifier,
      "libs": build_libs,
    })
  }

  #[op]
  fn op_cwd() -> String {
    "cache:///".into()
  }

  #[op]
  fn op_exists() -> bool {
    false
  }

  #[op]
  fn op_script_version(
    _state: &mut OpState,
    _args: Value,
  ) -> Result<Option<String>, AnyError> {
    Ok(Some("1".to_string()))
  }

  #[op]
  // using the same op that is used in `tsc.rs` for loading modules and reading
  // files, but a slightly different implementation at build time.
  fn op_load(state: &mut OpState, args: LoadArgs) -> Result<Value, AnyError> {
    let op_crate_libs = state.borrow::<HashMap<&str, PathBuf>>();
    let path_dts = state.borrow::<PathBuf>();
    let re_asset =
      Regex::new(r"asset:/{3}lib\.(\S+)\.d\.ts").expect("bad regex");
    let build_specifier = "asset:///bootstrap.ts";

    // we need a basic file to send to tsc to warm it up.
    if args.specifier == build_specifier {
      Ok(json!({
        "data": r#"console.log("hello deno!");"#,
        "version": "1",
        // this corresponds to `ts.ScriptKind.TypeScript`
        "scriptKind": 3
      }))
    // specifiers come across as `asset:///lib.{lib_name}.d.ts` and we need to
    // parse out just the name so we can lookup the asset.
    } else if let Some(caps) = re_asset.captures(&args.specifier) {
      if let Some(lib) = caps.get(1).map(|m| m.as_str()) {
        // if it comes from an op crate, we were supplied with the path to the
        // file.
        let path = if let Some(op_crate_lib) = op_crate_libs.get(lib) {
          PathBuf::from(op_crate_lib).canonicalize().unwrap()
        // otherwise we are will generate the path ourself
        } else {
          path_dts.join(format!("lib.{}.d.ts", lib))
        };
        let data = std::fs::read_to_string(path)?;
        Ok(json!({
          "data": data,
          "version": "1",
          // this corresponds to `ts.ScriptKind.TypeScript`
          "scriptKind": 3
        }))
      } else {
        Err(custom_error(
          "InvalidSpecifier",
          format!("An invalid specifier was requested: {}", args.specifier),
        ))
      }
    } else {
      Err(custom_error(
        "InvalidSpecifier",
        format!("An invalid specifier was requested: {}", args.specifier),
      ))
    }
  }

  Extension::builder()
    .ops(vec![
      op_build_info::decl(),
      op_cwd::decl(),
      op_exists::decl(),
      op_load::decl(),
      op_script_version::decl(),
    ])
    .state(move |state| {
      state.put(op_crate_libs.clone());
      state.put(build_libs.clone());
      state.put(path_dts.clone());

      Ok(())
    })
    .build()
}

fn deno_webgpu_get_declaration() -> PathBuf {
  cli_dir().join("dts").join("lib.deno_webgpu.d.ts")
}

fn load_js_files(js_runtime: &mut JsRuntime) {
  let js_files = get_js_files(tsc_dir());
  let cwd = cli_dir();
  let display_root = cwd.parent().unwrap();
  for file in js_files {
    println!("cargo:rerun-if-changed={}", file.display());
    let display_path = file.strip_prefix(display_root).unwrap();
    let display_path_str = display_path.display().to_string();
    js_runtime
      .execute_script(
        &("deno:".to_string() + &display_path_str.replace('\\', "/")),
        &std::fs::read_to_string(&file).unwrap(),
      )
      .unwrap();
  }
}

fn root_dir() -> PathBuf {
  // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the workspace root.
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("..")
    .canonicalize()
    .unwrap()
}

fn cli_dir() -> PathBuf {
  root_dir().join("cli")
}

fn tsc_dir() -> PathBuf {
  cli_dir().join("tsc")
}

fn get_js_files(dir: PathBuf) -> Vec<PathBuf> {
  let mut js_files = std::fs::read_dir(dir.clone())
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      dir.join(file.path())
    })
    .filter(|path| path.extension().unwrap_or_default() == "js")
    .collect::<Vec<PathBuf>>();
  js_files.sort();
  js_files
}
