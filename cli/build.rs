// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::Path;
use std::path::PathBuf;

use deno_core::snapshot_util::*;
use deno_core::Extension;
use deno_runtime::deno_cache::SqliteBackedCache;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::*;

mod ts {
  use super::*;
  use crate::deno_webgpu_get_declaration;
  use deno_core::error::custom_error;
  use deno_core::error::AnyError;
  use deno_core::op;
  use deno_core::OpState;
  use regex::Regex;
  use serde::Deserialize;
  use serde::Serialize;
  use serde_json::json;
  use serde_json::Value;
  use std::collections::HashMap;
  use std::path::Path;
  use std::path::PathBuf;

  #[derive(Debug, Deserialize)]
  struct LoadArgs {
    /// The fully qualified specifier that should be loaded.
    specifier: String,
  }

  #[derive(Clone, Serialize)]
  #[serde(rename_all = "camelCase")]
  struct LibFile {
    should_snapshot_parse: bool,
    name: String,
  }

  impl LibFile {
    /// Lib file that should be parsed in the snapshot.
    pub fn snapshot_parse(name: &str) -> Self {
      Self {
        should_snapshot_parse: true,
        name: name.to_string(),
      }
    }

    /// Lib file whose text should only exist in the snapshot
    /// and be parsed on demand.
    pub fn lazy_parse(name: &str) -> Self {
      Self {
        should_snapshot_parse: false,
        name: name.to_string(),
      }
    }
  }

  pub fn create_compiler_snapshot(
    snapshot_path: PathBuf,
    files: Vec<PathBuf>,
    cwd: &Path,
  ) {
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
      LibFile::snapshot_parse("deno.window"),
      LibFile::snapshot_parse("deno.worker"),
      LibFile::snapshot_parse("deno.shared_globals"),
      LibFile::snapshot_parse("deno.ns"),
      LibFile::snapshot_parse("deno.unstable"),
      // Deno built-in type libraries
      LibFile::snapshot_parse("es5"),
      LibFile::snapshot_parse("es6"),
      LibFile::snapshot_parse("es2015.collection"),
      LibFile::snapshot_parse("es2015.core"),
      LibFile::snapshot_parse("es2015"),
      LibFile::snapshot_parse("es2015.generator"),
      LibFile::snapshot_parse("es2015.iterable"),
      LibFile::snapshot_parse("es2015.promise"),
      LibFile::snapshot_parse("es2015.proxy"),
      LibFile::snapshot_parse("es2015.reflect"),
      LibFile::snapshot_parse("es2015.symbol"),
      LibFile::snapshot_parse("es2015.symbol.wellknown"),
      LibFile::snapshot_parse("es2016.array.include"),
      LibFile::snapshot_parse("es2016"),
      LibFile::snapshot_parse("es2017"),
      LibFile::snapshot_parse("es2017.intl"),
      LibFile::snapshot_parse("es2017.object"),
      LibFile::snapshot_parse("es2017.sharedmemory"),
      LibFile::snapshot_parse("es2017.string"),
      LibFile::snapshot_parse("es2017.typedarrays"),
      LibFile::snapshot_parse("es2018.asyncgenerator"),
      LibFile::snapshot_parse("es2018.asynciterable"),
      LibFile::snapshot_parse("es2018"),
      LibFile::snapshot_parse("es2018.intl"),
      LibFile::snapshot_parse("es2018.promise"),
      LibFile::snapshot_parse("es2018.regexp"),
      LibFile::snapshot_parse("es2019.array"),
      LibFile::snapshot_parse("es2019"),
      LibFile::snapshot_parse("es2019.intl"),
      LibFile::snapshot_parse("es2019.object"),
      LibFile::snapshot_parse("es2019.string"),
      LibFile::snapshot_parse("es2019.symbol"),
      LibFile::snapshot_parse("es2020.bigint"),
      LibFile::snapshot_parse("es2020"),
      LibFile::snapshot_parse("es2020.date"),
      LibFile::snapshot_parse("es2020.intl"),
      LibFile::snapshot_parse("es2020.number"),
      LibFile::snapshot_parse("es2020.promise"),
      LibFile::snapshot_parse("es2020.sharedmemory"),
      LibFile::snapshot_parse("es2020.string"),
      LibFile::snapshot_parse("es2020.symbol.wellknown"),
      LibFile::snapshot_parse("es2021"),
      LibFile::snapshot_parse("es2021.intl"),
      LibFile::snapshot_parse("es2021.promise"),
      LibFile::snapshot_parse("es2021.string"),
      LibFile::snapshot_parse("es2021.weakref"),
      LibFile::snapshot_parse("es2022"),
      LibFile::snapshot_parse("es2022.array"),
      LibFile::snapshot_parse("es2022.error"),
      LibFile::snapshot_parse("es2022.intl"),
      LibFile::snapshot_parse("es2022.object"),
      LibFile::snapshot_parse("es2022.sharedmemory"),
      LibFile::snapshot_parse("es2022.string"),
      LibFile::snapshot_parse("esnext"),
      LibFile::snapshot_parse("esnext.array"),
      LibFile::snapshot_parse("esnext.full"),
      LibFile::snapshot_parse("esnext.intl"),
      // Libraries that should be parsed on demand
      LibFile::lazy_parse("dom.asynciterable"),
      LibFile::lazy_parse("dom"),
      LibFile::lazy_parse("dom.extras"),
      LibFile::lazy_parse("dom.iterable"),
      LibFile::lazy_parse("es6"),
      LibFile::lazy_parse("es2016.full"),
      LibFile::lazy_parse("es2017.full"),
      LibFile::lazy_parse("es2018.full"),
      LibFile::lazy_parse("es2019.full"),
      LibFile::lazy_parse("es2020.full"),
      LibFile::lazy_parse("es2021.full"),
      LibFile::lazy_parse("es2022.full"),
      LibFile::lazy_parse("esnext.full"),
      LibFile::lazy_parse("scripthost"),
      LibFile::lazy_parse("webworker"),
      LibFile::lazy_parse("webworker.importscripts"),
      LibFile::lazy_parse("webworker.iterable"),
    ];

    let path_dts = cwd.join("tsc/dts");
    // ensure we invalidate the build properly.
    for lib in libs.iter() {
      println!(
        "cargo:rerun-if-changed={}",
        path_dts.join(format!("lib.{}.d.ts", lib.name)).display()
      );
    }
    println!(
      "cargo:rerun-if-changed={}",
      cwd.join("tsc").join("00_typescript.js").display()
    );
    println!(
      "cargo:rerun-if-changed={}",
      cwd.join("tsc").join("99_main_compiler.js").display()
    );
    println!(
      "cargo:rerun-if-changed={}",
      cwd.join("js").join("40_testing.js").display()
    );

    // create a copy of the vector that includes any op crate libs to be passed
    // to the JavaScript compiler to build into the snapshot
    let mut build_libs = libs;
    for (op_lib_name, _) in op_crate_libs.iter() {
      build_libs.push(LibFile::snapshot_parse(op_lib_name));
    }

    #[op]
    fn op_build_info(state: &mut OpState) -> Value {
      let build_specifier = "asset:///bootstrap.ts";
      let build_libs = state.borrow::<Vec<LibFile>>();
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
    fn op_is_node_file() -> bool {
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

    create_snapshot(CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      snapshot_path,
      startup_snapshot: None,
      extensions: vec![Extension::builder("deno_tsc")
        .ops(vec![
          op_build_info::decl(),
          op_cwd::decl(),
          op_exists::decl(),
          op_is_node_file::decl(),
          op_load::decl(),
          op_script_version::decl(),
        ])
        .state(move |state| {
          state.put(op_crate_libs.clone());
          state.put(build_libs.clone());
          state.put(path_dts.clone());

          Ok(())
        })
        .build()],
      extensions_with_js: vec![],
      additional_files: files,
      compression_cb: Some(Box::new(|vec, snapshot_slice| {
        vec.extend_from_slice(
          &zstd::bulk::compress(snapshot_slice, 22)
            .expect("snapshot compression failed"),
        );
      })),
    });
  }

  pub(crate) fn version() -> String {
    let file_text = std::fs::read_to_string("tsc/00_typescript.js").unwrap();
    let mut version = String::new();
    for line in file_text.lines() {
      let major_minor_text = "ts.versionMajorMinor = \"";
      let version_text = "ts.version = \"\".concat(ts.versionMajorMinor, \"";
      if version.is_empty() {
        if let Some(index) = line.find(major_minor_text) {
          let remaining_line = &line[index + major_minor_text.len()..];
          version
            .push_str(&remaining_line[..remaining_line.find('"').unwrap()]);
        }
      } else if let Some(index) = line.find(version_text) {
        let remaining_line = &line[index + version_text.len()..];
        version.push_str(&remaining_line[..remaining_line.find('"').unwrap()]);
        return version;
      }
    }
    panic!("Could not find ts version.")
  }
}

fn create_cli_snapshot(snapshot_path: PathBuf, files: Vec<PathBuf>) {
  let extensions: Vec<Extension> = vec![
    deno_webidl::init(),
    deno_console::init(),
    deno_url::init(),
    deno_tls::init(),
    deno_web::init::<PermissionsContainer>(
      deno_web::BlobStore::default(),
      Default::default(),
    ),
    deno_fetch::init::<PermissionsContainer>(Default::default()),
    deno_cache::init::<SqliteBackedCache>(None),
    deno_websocket::init::<PermissionsContainer>("".to_owned(), None, None),
    deno_webstorage::init(None),
    deno_crypto::init(None),
    deno_webgpu::init(false),
    deno_broadcast_channel::init(
      deno_broadcast_channel::InMemoryBroadcastChannel::default(),
      false, // No --unstable.
    ),
    deno_node::init::<PermissionsContainer>(None), // No --unstable.
    deno_ffi::init::<PermissionsContainer>(false),
    deno_net::init::<PermissionsContainer>(
      None, false, // No --unstable.
      None,
    ),
    deno_napi::init::<PermissionsContainer>(false),
    deno_http::init(),
    deno_flash::init::<PermissionsContainer>(false), // No --unstable
  ];

  create_snapshot(CreateSnapshotOptions {
    cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
    snapshot_path,
    startup_snapshot: Some(deno_runtime::js::deno_isolate_init()),
    extensions,
    extensions_with_js: vec![],
    additional_files: files,
    compression_cb: Some(Box::new(|vec, snapshot_slice| {
      lzzzz::lz4_hc::compress_to_vec(
        snapshot_slice,
        vec,
        lzzzz::lz4_hc::CLEVEL_MAX,
      )
      .expect("snapshot compression failed");
    })),
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
  if target != host {
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
    let ver = glibc_version::get_version().unwrap();

    // If a custom compiler is set, the glibc version is not reliable.
    // Here, we assume that if a custom compiler is used, that it will be modern enough to support a dynamic symbol list.
    if env::var("CC").is_err() && ver.major <= 2 && ver.minor < 35 {
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
    println!("cargo:rustc-env=DENO_CANARY={}", c);
  }
  println!("cargo:rerun-if-env-changed=DENO_CANARY");

  println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_commit_hash());
  println!("cargo:rerun-if-env-changed=GIT_COMMIT_HASH");

  println!("cargo:rustc-env=TS_VERSION={}", ts::version());
  println!("cargo:rerun-if-env-changed=TS_VERSION");

  println!(
    "cargo:rustc-env=DENO_CONSOLE_LIB_PATH={}",
    deno_console::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_URL_LIB_PATH={}",
    deno_url::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEB_LIB_PATH={}",
    deno_web::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_FETCH_LIB_PATH={}",
    deno_fetch::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBGPU_LIB_PATH={}",
    deno_webgpu_get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBSOCKET_LIB_PATH={}",
    deno_websocket::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBSTORAGE_LIB_PATH={}",
    deno_webstorage::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_CACHE_LIB_PATH={}",
    deno_cache::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_CRYPTO_LIB_PATH={}",
    deno_crypto::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_BROADCAST_CHANNEL_LIB_PATH={}",
    deno_broadcast_channel::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_NET_LIB_PATH={}",
    deno_net::get_declaration().display()
  );

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  let compiler_snapshot_path = o.join("COMPILER_SNAPSHOT.bin");
  let js_files = get_js_files(env!("CARGO_MANIFEST_DIR"), "tsc");
  ts::create_compiler_snapshot(compiler_snapshot_path, js_files, &c);

  let cli_snapshot_path = o.join("CLI_SNAPSHOT.bin");
  let mut js_files = get_js_files(env!("CARGO_MANIFEST_DIR"), "js");
  js_files.push(deno_runtime::js::get_99_main());
  create_cli_snapshot(cli_snapshot_path, js_files);

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
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  manifest_dir
    .join("tsc")
    .join("dts")
    .join("lib.deno_webgpu.d.ts")
}
