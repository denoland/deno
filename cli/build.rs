// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::PathBuf;

use deno_core::snapshot_util::*;
use deno_core::ExtensionFileSource;
use deno_core::ExtensionFileSourceCode;
use deno_runtime::*;

mod ts {
  use super::*;
  use deno_core::error::custom_error;
  use deno_core::error::AnyError;
  use deno_core::op2;
  use deno_core::OpState;
  use deno_runtime::deno_node::SUPPORTED_BUILTIN_NODE_MODULES;
  use serde::Deserialize;
  use serde::Serialize;
  use std::collections::HashMap;
  use std::path::Path;
  use std::path::PathBuf;

  #[derive(Debug, Deserialize)]
  struct LoadArgs {
    /// The fully qualified specifier that should be loaded.
    specifier: String,
  }

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]
  struct BuildInfoResponse {
    build_specifier: String,
    libs: Vec<String>,
    node_built_in_module_names: Vec<String>,
  }

  #[op2]
  #[serde]
  fn op_build_info(state: &mut OpState) -> BuildInfoResponse {
    let build_specifier = "asset:///bootstrap.ts".to_string();
    let build_libs = state
      .borrow::<Vec<&str>>()
      .iter()
      .map(|s| s.to_string())
      .collect();
    let node_built_in_module_names = SUPPORTED_BUILTIN_NODE_MODULES
      .iter()
      .map(|s| s.to_string())
      .collect();
    BuildInfoResponse {
      build_specifier,
      libs: build_libs,
      node_built_in_module_names,
    }
  }

  #[op2(fast)]
  fn op_is_node_file() -> bool {
    false
  }

  #[derive(Debug, Deserialize, Serialize)]
  #[serde(rename_all = "camelCase")]
  struct ScriptVersionArgs {
    specifier: String,
  }

  #[op2]
  #[string]
  fn op_script_version(
    _state: &mut OpState,
    #[serde] _args: ScriptVersionArgs,
  ) -> Result<Option<String>, AnyError> {
    Ok(Some("1".to_string()))
  }

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
    #[serde] args: LoadArgs,
  ) -> Result<LoadResponse, AnyError> {
    let op_crate_libs = state.borrow::<HashMap<&str, PathBuf>>();
    let path_dts = state.borrow::<PathBuf>();
    let re_asset = lazy_regex::regex!(r"asset:/{3}lib\.(\S+)\.d\.ts");
    let build_specifier = "asset:///bootstrap.ts";

    // we need a basic file to send to tsc to warm it up.
    if args.specifier == build_specifier {
      Ok(LoadResponse {
        data: r#"Deno.writeTextFile("hello.txt", "hello deno!");"#.to_string(),
        version: "1".to_string(),
        // this corresponds to `ts.ScriptKind.TypeScript`
        script_kind: 3,
      })
      // specifiers come across as `asset:///lib.{lib_name}.d.ts` and we need to
      // parse out just the name so we can lookup the asset.
    } else if let Some(caps) = re_asset.captures(&args.specifier) {
      if let Some(lib) = caps.get(1).map(|m| m.as_str()) {
        // if it comes from an op crate, we were supplied with the path to the
        // file.
        let path = if let Some(op_crate_lib) = op_crate_libs.get(lib) {
          PathBuf::from(op_crate_lib).canonicalize()?
          // otherwise we will generate the path ourself
        } else {
          path_dts.join(format!("lib.{lib}.d.ts"))
        };
        let data = std::fs::read_to_string(path)?;
        Ok(LoadResponse {
          data,
          version: "1".to_string(),
          // this corresponds to `ts.ScriptKind.TypeScript`
          script_kind: 3,
        })
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

  deno_core::extension!(deno_tsc,
    ops = [op_build_info, op_is_node_file, op_load, op_script_version],
    js = [
      dir "tsc",
      "00_typescript.js",
      "99_main_compiler.js",
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
      "esnext",
      "esnext.array",
      "esnext.decorators",
      "esnext.disposable",
      "esnext.intl",
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

    let output = create_snapshot(CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      snapshot_path,
      startup_snapshot: None,
      extensions: vec![deno_tsc::init_ops_and_esm(
        op_crate_libs,
        build_libs,
        path_dts,
      )],

      // NOTE(bartlomieju): Compressing the TSC snapshot in debug build took
      // ~45s on M1 MacBook Pro; without compression it took ~1s.
      // Thus we're not not using compressed snapshot, trading off
      // a lot of build time for some startup time in debug build.
      #[cfg(debug_assertions)]
      compression_cb: None,

      #[cfg(not(debug_assertions))]
      compression_cb: Some(Box::new(|vec, snapshot_slice| {
        eprintln!("Compressing TSC snapshot...");
        vec.extend_from_slice(
          &zstd::bulk::compress(snapshot_slice, 22)
            .expect("snapshot compression failed"),
        );
      })),
      with_runtime_cb: None,
    });
    for path in output.files_loaded_during_snapshot {
      println!("cargo:rerun-if-changed={}", path.display());
    }
  }

  pub(crate) fn version() -> String {
    let file_text = std::fs::read_to_string("tsc/00_typescript.js").unwrap();
    let version_text = "  version = \"";
    for line in file_text.lines() {
      if let Some(index) = line.find(version_text) {
        let remaining_line = &line[index + version_text.len()..];
        return remaining_line[..remaining_line.find('"').unwrap()].to_string();
      }
    }
    panic!("Could not find ts version.")
  }
}

// Duplicated in `ops/mod.rs`. Keep in sync!
deno_core::extension!(
  cli,
  deps = [runtime],
  esm_entry_point = "ext:cli/99_main.js",
  esm = [
    dir "js",
    "40_testing.js",
    "99_main.js"
  ],
  customizer = |ext: &mut deno_core::Extension| {
    ext.esm_files.to_mut().push(ExtensionFileSource {
      specifier: "ext:cli/runtime/js/99_main.js",
      code: ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(
        deno_runtime::js::PATH_FOR_99_MAIN_JS,
      ),
    });
  }
);

#[cfg(not(feature = "__runtime_js_sources"))]
#[must_use = "The files listed by create_cli_snapshot should be printed as 'cargo:rerun-if-changed' lines"]
fn create_cli_snapshot(snapshot_path: PathBuf) -> CreateSnapshotOutput {
  use deno_core::Extension;
  use deno_runtime::deno_cache::SqliteBackedCache;
  use deno_runtime::deno_http::DefaultHttpPropertyExtractor;
  use deno_runtime::deno_kv::sqlite::SqliteDbHandler;
  use deno_runtime::permissions::PermissionsContainer;
  use std::sync::Arc;

  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs` and `runtime/build.rs`!
  let fs = Arc::new(deno_fs::RealFs);
  let extensions: Vec<Extension> = vec![
    deno_webidl::deno_webidl::init_ops(),
    deno_console::deno_console::init_ops(),
    deno_url::deno_url::init_ops(),
    deno_web::deno_web::init_ops::<PermissionsContainer>(
      Default::default(),
      Default::default(),
    ),
    deno_fetch::deno_fetch::init_ops::<PermissionsContainer>(Default::default()),
    deno_cache::deno_cache::init_ops::<SqliteBackedCache>(None),
    deno_websocket::deno_websocket::init_ops::<PermissionsContainer>(
      "".to_owned(),
      None,
      None,
    ),
    deno_webstorage::deno_webstorage::init_ops(None),
    deno_crypto::deno_crypto::init_ops(None),
    deno_broadcast_channel::deno_broadcast_channel::init_ops(
      deno_broadcast_channel::InMemoryBroadcastChannel::default(),
      false, // No --unstable.
    ),
    deno_ffi::deno_ffi::init_ops::<PermissionsContainer>(false),
    deno_net::deno_net::init_ops::<PermissionsContainer>(
      None, false, // No --unstable.
      None,
    ),
    deno_tls::deno_tls::init_ops(),
    deno_kv::deno_kv::init_ops(
      SqliteDbHandler::<PermissionsContainer>::new(None),
      false, // No --unstable.
    ),
    deno_napi::deno_napi::init_ops::<PermissionsContainer>(),
    deno_http::deno_http::init_ops::<DefaultHttpPropertyExtractor>(),
    deno_io::deno_io::init_ops(Default::default()),
    deno_fs::deno_fs::init_ops::<PermissionsContainer>(false, fs.clone()),
    deno_node::deno_node::init_ops::<PermissionsContainer>(None, fs),
    deno_runtime::runtime::init_ops(),
    cli::init_ops_and_esm(), // NOTE: This needs to be init_ops_and_esm!
  ];

  create_snapshot(CreateSnapshotOptions {
    cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
    snapshot_path,
    startup_snapshot: deno_runtime::js::deno_isolate_init(),
    extensions,
    compression_cb: None,
    with_runtime_cb: None,
  })
}

fn git_commit_hash() -> String {
  if let Ok(output) = std::process::Command::new("git")
    .arg("rev-list")
    .arg("-1")
    .arg("HEAD")
    .output()
  {
    if output.status.success() {
      std::str::from_utf8(&output.stdout[..40])
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
  // Skip building from docs.rs.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }

  // Host snapshots won't work when cross compiling.
  let target = env::var("TARGET").unwrap();
  let host = env::var("HOST").unwrap();
  let skip_cross_check =
    env::var("DENO_SKIP_CROSS_BUILD_CHECK").map_or(false, |v| v == "1");
  if !skip_cross_check && target != host {
    panic!("Cross compiling with snapshot is not supported.");
  }

  let symbols_path = std::path::Path::new("napi").join(
    format!("generated_symbol_exports_list_{}.def", env::consts::OS).as_str(),
  )
  .canonicalize()
  .expect(
    "Missing symbols list! Generate using tools/napi/generate_symbols_lists.js",
  );

  #[cfg(target_os = "windows")]
  println!(
    "cargo:rustc-link-arg-bin=deno=/DEF:{}",
    symbols_path.display()
  );

  #[cfg(target_os = "macos")]
  println!(
    "cargo:rustc-link-arg-bin=deno=-Wl,-exported_symbols_list,{}",
    symbols_path.display()
  );

  #[cfg(target_os = "linux")]
  {
    // If a custom compiler is set, the glibc version is not reliable.
    // Here, we assume that if a custom compiler is used, that it will be modern enough to support a dynamic symbol list.
    if env::var("CC").is_err()
      && glibc_version::get_version()
        .map(|ver| ver.major <= 2 && ver.minor < 35)
        .unwrap_or(false)
    {
      println!("cargo:warning=Compiling with all symbols exported, this will result in a larger binary. Please use glibc 2.35 or later for an optimised build.");
      println!("cargo:rustc-link-arg-bin=deno=-rdynamic");
    } else {
      println!(
        "cargo:rustc-link-arg-bin=deno=-Wl,--export-dynamic-symbol-list={}",
        symbols_path.display()
      );
    }
  }

  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  if let Ok(c) = env::var("DENO_CANARY") {
    println!("cargo:rustc-env=DENO_CANARY={c}");
  }
  println!("cargo:rerun-if-env-changed=DENO_CANARY");

  println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_commit_hash());
  println!("cargo:rerun-if-env-changed=GIT_COMMIT_HASH");
  println!(
    "cargo:rustc-env=GIT_COMMIT_HASH_SHORT={}",
    &git_commit_hash()[..7]
  );

  let ts_version = ts::version();
  debug_assert_eq!(ts_version, "5.2.2"); // bump this assertion when it changes
  println!("cargo:rustc-env=TS_VERSION={}", ts_version);
  println!("cargo:rerun-if-env-changed=TS_VERSION");

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  let compiler_snapshot_path = o.join("COMPILER_SNAPSHOT.bin");
  ts::create_compiler_snapshot(compiler_snapshot_path, &c);

  #[cfg(not(feature = "__runtime_js_sources"))]
  {
    let cli_snapshot_path = o.join("CLI_SNAPSHOT.bin");
    let output = create_cli_snapshot(cli_snapshot_path);
    for path in output.files_loaded_during_snapshot {
      println!("cargo:rerun-if-changed={}", path.display())
    }
  }

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
