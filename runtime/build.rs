// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::PathBuf;

#[cfg(all(
  not(feature = "docsrs"),
  not(feature = "dont_create_runtime_snapshot")
))]
mod startup_snapshot {
  use super::*;
  use deno_ast::MediaType;
  use deno_ast::ParseParams;
  use deno_ast::SourceTextInfo;
  use deno_cache::SqliteBackedCache;
  use deno_core::error::AnyError;
  use deno_core::snapshot_util::*;
  use deno_core::Extension;
  use deno_core::ExtensionFileSource;
  use deno_core::ModuleCode;
  use deno_fs::StdFs;
  use std::path::Path;

  fn transpile_ts_for_snapshotting(
    file_source: &ExtensionFileSource,
  ) -> Result<ModuleCode, AnyError> {
    let media_type = MediaType::from_path(Path::new(&file_source.specifier));

    let should_transpile = match media_type {
      MediaType::JavaScript => false,
      MediaType::Mjs => false,
      MediaType::TypeScript => true,
      _ => panic!(
        "Unsupported media type for snapshotting {media_type:?} for file {}",
        file_source.specifier
      ),
    };
    let code = file_source.load()?;

    if !should_transpile {
      return Ok(code);
    }

    let parsed = deno_ast::parse_module(ParseParams {
      specifier: file_source.specifier.to_string(),
      text_info: SourceTextInfo::from_string(code.as_str().to_owned()),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })?;
    let transpiled_source = parsed.transpile(&deno_ast::EmitOptions {
      imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
      inline_source_map: false,
      ..Default::default()
    })?;

    Ok(transpiled_source.text.into())
  }

  #[derive(Clone)]
  struct Permissions;

  impl deno_fetch::FetchPermissions for Permissions {
    fn check_net_url(
      &mut self,
      _url: &deno_core::url::Url,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read(
      &mut self,
      _p: &Path,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_websocket::WebSocketPermissions for Permissions {
    fn check_net_url(
      &mut self,
      _url: &deno_core::url::Url,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_web::TimersPermission for Permissions {
    fn allow_hrtime(&mut self) -> bool {
      unreachable!("snapshotting!")
    }

    fn check_unstable(
      &self,
      _state: &deno_core::OpState,
      _api_name: &'static str,
    ) {
      unreachable!("snapshotting!")
    }
  }

  impl deno_ffi::FfiPermissions for Permissions {
    fn check(
      &mut self,
      _path: Option<&Path>,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_napi::NapiPermissions for Permissions {
    fn check(
      &mut self,
      _path: Option<&Path>,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_node::NodePermissions for Permissions {
    fn check_read(
      &mut self,
      _p: &Path,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_net::NetPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
      &mut self,
      _host: &(T, Option<u16>),
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read(
      &mut self,
      _p: &Path,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write(
      &mut self,
      _p: &Path,
      _api_name: &str,
    ) -> Result<(), deno_core::error::AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_fs::FsPermissions for Permissions {
    fn check_read(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_read_blind(
      &mut self,
      _path: &Path,
      _display: &str,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write_blind(
      &mut self,
      _path: &Path,
      _display: &str,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }
  }

  impl deno_kv::sqlite::SqliteDbHandlerPermissions for Permissions {
    fn check_read(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write(
      &mut self,
      _path: &Path,
      _api_name: &str,
    ) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }
  }

  struct SnapshotNodeEnv;

  impl deno_node::NodeEnv for SnapshotNodeEnv {
    type P = Permissions;
    type Fs = deno_node::RealFs;
  }

  deno_core::extension!(runtime,
    deps = [
      deno_webidl,
      deno_console,
      deno_url,
      deno_tls,
      deno_web,
      deno_fetch,
      deno_cache,
      deno_websocket,
      deno_webstorage,
      deno_crypto,
      deno_broadcast_channel,
      // FIXME(bartlomieju): this should be reenabled
      // "deno_node",
      deno_ffi,
      deno_net,
      deno_napi,
      deno_http,
      deno_io,
      deno_fs
    ],
    esm = [
      dir "js",
      "01_errors.js",
      "01_version.ts",
      "06_util.js",
      "10_permissions.js",
      "11_workers.js",
      "13_buffer.js",
      "30_os.js",
      "40_fs_events.js",
      "40_http.js",
      "40_process.js",
      "40_signals.js",
      "40_tty.js",
      "41_prompt.js",
      "90_deno_ns.js",
      "98_global_scope.js"
    ],
  );

  #[cfg(not(feature = "snapshot_from_snapshot"))]
  deno_core::extension!(
    runtime_main,
    deps = [runtime],
    customizer = |ext: &mut deno_core::ExtensionBuilder| {
      ext.esm(vec![ExtensionFileSource {
        specifier: "ext:runtime_main/js/99_main.js",
        code: deno_core::ExtensionFileSourceCode::IncludedInBinary(
          include_str!("js/99_main.js"),
        ),
      }]);
    }
  );

  pub fn create_runtime_snapshot(snapshot_path: PathBuf) {
    // NOTE(bartlomieju): ordering is important here, keep it in sync with
    // `runtime/worker.rs`, `runtime/web_worker.rs` and `cli/build.rs`!
    let extensions: Vec<Extension> = vec![
      deno_webidl::deno_webidl::init_ops_and_esm(),
      deno_console::deno_console::init_ops_and_esm(),
      deno_url::deno_url::init_ops_and_esm(),
      deno_web::deno_web::init_ops_and_esm::<Permissions>(
        deno_web::BlobStore::default(),
        Default::default(),
      ),
      deno_fetch::deno_fetch::init_ops_and_esm::<Permissions>(
        Default::default(),
      ),
      deno_cache::deno_cache::init_ops_and_esm::<SqliteBackedCache>(None),
      deno_websocket::deno_websocket::init_ops_and_esm::<Permissions>(
        "".to_owned(),
        None,
        None,
      ),
      deno_webstorage::deno_webstorage::init_ops_and_esm(None),
      deno_crypto::deno_crypto::init_ops_and_esm(None),
      deno_broadcast_channel::deno_broadcast_channel::init_ops_and_esm(
        deno_broadcast_channel::InMemoryBroadcastChannel::default(),
        false, // No --unstable.
      ),
      deno_ffi::deno_ffi::init_ops_and_esm::<Permissions>(false),
      deno_net::deno_net::init_ops_and_esm::<Permissions>(
        None, false, // No --unstable.
        None,
      ),
      deno_tls::deno_tls::init_ops_and_esm(),
      deno_kv::deno_kv::init_ops_and_esm(
        deno_kv::sqlite::SqliteDbHandler::<Permissions>::new(None),
        false, // No --unstable
      ),
      deno_napi::deno_napi::init_ops_and_esm::<Permissions>(),
      deno_http::deno_http::init_ops_and_esm(),
      deno_io::deno_io::init_ops_and_esm(Default::default()),
      deno_fs::deno_fs::init_ops_and_esm::<_, Permissions>(false, StdFs),
      runtime::init_ops_and_esm(),
      // FIXME(bartlomieju): these extensions are specified last, because they
      // depend on `runtime`, even though it should be other way around
      deno_node::deno_node::init_ops_and_esm::<SnapshotNodeEnv>(None),
      #[cfg(not(feature = "snapshot_from_snapshot"))]
      runtime_main::init_ops_and_esm(),
    ];

    create_snapshot(CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      snapshot_path,
      startup_snapshot: None,
      extensions,
      compression_cb: None,
      snapshot_module_load_cb: Some(Box::new(transpile_ts_for_snapshotting)),
    });
  }
}

fn main() {
  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let runtime_snapshot_path = o.join("RUNTIME_SNAPSHOT.bin");

  // If we're building on docs.rs we just create
  // and empty snapshot file and return, because `rusty_v8`
  // doesn't actually compile on docs.rs
  if env::var_os("DOCS_RS").is_some() {
    let snapshot_slice = &[];
    #[allow(clippy::needless_borrow)]
    std::fs::write(&runtime_snapshot_path, snapshot_slice).unwrap();
  }

  #[cfg(all(
    not(feature = "docsrs"),
    not(feature = "dont_create_runtime_snapshot")
  ))]
  startup_snapshot::create_runtime_snapshot(runtime_snapshot_path)
}
