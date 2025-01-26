// Copyright 2018-2025 the Deno authors. MIT license.

use std::env;
use std::path::PathBuf;

use deno_core::snapshot::*;
use deno_runtime::*;

mod ts {
  use std::collections::HashMap;
  use std::io::Write;
  use std::path::Path;
  use std::path::PathBuf;

  use deno_core::op2;
  use deno_core::v8;
  use deno_core::OpState;
  use deno_error::JsErrorBox;
  use serde::Serialize;

  use super::*;

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]
  struct LoadResponse {
    data: String,
    version: String,
    script_kind: i32,
  }

  #[op2]
  #[serde]
  // using the same op that is used in `tsc.rs` for loading modules and reading
  // files, but a slightly different implementation at build time.
  fn op_load(
    state: &mut OpState,
    #[string] load_specifier: &str,
  ) -> Result<LoadResponse, JsErrorBox> {
    let op_crate_libs = state.borrow::<HashMap<&str, PathBuf>>();
    let path_dts = state.borrow::<PathBuf>();
    let re_asset = lazy_regex::regex!(r"asset:/{3}lib\.(\S+)\.d\.ts");

    // specifiers come across as `asset:///lib.{lib_name}.d.ts` and we need to
    // parse out just the name so we can lookup the asset.
    if let Some(caps) = re_asset.captures(load_specifier) {
      if let Some(lib) = caps.get(1).map(|m| m.as_str()) {
        // if it comes from an op crate, we were supplied with the path to the
        // file.
        let path = if let Some(op_crate_lib) = op_crate_libs.get(lib) {
          PathBuf::from(op_crate_lib)
            .canonicalize()
            .map_err(JsErrorBox::from_err)?
          // otherwise we will generate the path ourself
        } else {
          path_dts.join(format!("lib.{lib}.d.ts"))
        };
        let data =
          std::fs::read_to_string(path).map_err(JsErrorBox::from_err)?;
        return Ok(LoadResponse {
          data,
          version: "1".to_string(),
          // this corresponds to `ts.ScriptKind.TypeScript`
          script_kind: 3,
        });
      }
    }

    Err(JsErrorBox::new(
      "InvalidSpecifier",
      format!("An invalid specifier was requested: {}", load_specifier),
    ))
  }

  deno_core::extension!(deno_tsc,
    ops = [
      op_load,
    ],
    esm_entry_point = "ext:deno_tsc/99_main_compiler.js",
    esm = [
      dir "tsc",
      "97_ts_host.js",
      "98_lsp.js",
      "99_main_compiler.js",
    ],
    js = [
      dir "tsc",
      "00_typescript.js",
    ],
    options = {
      op_crate_libs: HashMap<&'static str, PathBuf>,
      build_libs: Vec<&'static str>,
      path_dts: PathBuf,
    },
    state = |state, options| {
      state.put(options.op_crate_libs);
      state.put(options.build_libs);
      state.put(options.path_dts);
    },
  );

  pub fn create_compiler_snapshot(snapshot_path: PathBuf, cwd: &Path) {
    // libs that are being provided by op crates.
    let mut op_crate_libs = HashMap::new();
    op_crate_libs.insert("deno.cache", deno_cache::get_declaration());
    op_crate_libs.insert("deno.console", deno_console::get_declaration());
    op_crate_libs.insert("deno.url", deno_url::get_declaration());
    op_crate_libs.insert("deno.web", deno_web::get_declaration());
    op_crate_libs.insert("deno.fetch", deno_fetch::get_declaration());
    op_crate_libs.insert("deno.webgpu", deno_webgpu_get_declaration());
    op_crate_libs.insert("deno.websocket", deno_websocket::get_declaration());
    op_crate_libs.insert("deno.webstorage", deno_webstorage::get_declaration());
    op_crate_libs.insert("deno.canvas", deno_canvas::get_declaration());
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
      "decorators",
      "decorators.legacy",
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
      "es2016.intl",
      "es2016",
      "es2017",
      "es2017.date",
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
      "es2019.intl",
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
      "es2022.regexp",
      "es2022.sharedmemory",
      "es2022.string",
      "es2023",
      "es2023.array",
      "es2023.collection",
      "es2023.intl",
      "esnext",
      "esnext.array",
      "esnext.collection",
      "esnext.decorators",
      "esnext.disposable",
      "esnext.intl",
      "esnext.iterator",
      "esnext.object",
      "esnext.promise",
      "esnext.regexp",
      "esnext.string",
    ];

    let path_dts = cwd.join("tsc/dts");
    // ensure we invalidate the build properly.
    for name in libs.iter() {
      println!(
        "cargo:rerun-if-changed={}",
        path_dts.join(format!("lib.{name}.d.ts")).display()
      );
    }

    // create a copy of the vector that includes any op crate libs to be passed
    // to the JavaScript compiler to build into the snapshot
    let mut build_libs = libs.clone();
    for (op_lib, _) in op_crate_libs.iter() {
      build_libs.push(op_lib.to_owned());
    }

    // used in the tests to verify that after snapshotting it has the same number
    // of lib files loaded and hasn't included any ones lazily loaded from Rust
    std::fs::write(
      PathBuf::from(env::var_os("OUT_DIR").unwrap())
        .join("lib_file_names.json"),
      serde_json::to_string(&build_libs).unwrap(),
    )
    .unwrap();

    // Leak to satisfy type-checker. It's okay since it's only run once for a build script.
    let build_libs_ = Box::leak(Box::new(build_libs.clone()));
    let runtime_cb = Box::new(|rt: &mut deno_core::JsRuntimeForSnapshot| {
      let scope = &mut rt.handle_scope();

      let context = scope.get_current_context();
      let global = context.global(scope);

      let name = v8::String::new(scope, "snapshot").unwrap();
      let snapshot_fn_val = global.get(scope, name.into()).unwrap();
      let snapshot_fn: v8::Local<v8::Function> =
        snapshot_fn_val.try_into().unwrap();
      let undefined = v8::undefined(scope);
      let build_libs = build_libs_.clone();
      let build_libs_v8 =
        deno_core::serde_v8::to_v8(scope, build_libs).unwrap();

      snapshot_fn
        .call(scope, undefined.into(), &[build_libs_v8])
        .unwrap();
    });

    let output = create_snapshot(
      CreateSnapshotOptions {
        cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
        startup_snapshot: None,
        extensions: vec![deno_tsc::init_ops_and_esm(
          op_crate_libs,
          build_libs,
          path_dts,
        )],
        extension_transpiler: None,
        with_runtime_cb: Some(runtime_cb),
        skip_op_registration: false,
      },
      None,
    )
    .unwrap();

    // NOTE(bartlomieju): Compressing the TSC snapshot in debug build took
    // ~45s on M1 MacBook Pro; without compression it took ~1s.
    // Thus we're not using compressed snapshot, trading off
    // a lot of build time for some startup time in debug build.
    let mut file = std::fs::File::create(snapshot_path).unwrap();
    if cfg!(debug_assertions) {
      file.write_all(&output.output).unwrap();
    } else {
      let mut vec = Vec::with_capacity(output.output.len());
      vec.extend((output.output.len() as u32).to_le_bytes());
      vec.extend_from_slice(
        &zstd::bulk::compress(&output.output, 22)
          .expect("snapshot compression failed"),
      );
      file.write_all(&vec).unwrap();
    }

    for path in output.files_loaded_during_snapshot {
      println!("cargo:rerun-if-changed={}", path.display());
    }
  }
}

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

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  let compiler_snapshot_path = o.join("COMPILER_SNAPSHOT.bin");
  ts::create_compiler_snapshot(compiler_snapshot_path, &c);

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

fn deno_webgpu_get_declaration() -> PathBuf {
  let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  manifest_dir
    .join("tsc")
    .join("dts")
    .join("lib.deno_webgpu.d.ts")
}
