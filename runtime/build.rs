// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::PathBuf;

#[cfg(all(
  not(feature = "docsrs"),
  not(feature = "dont_create_runtime_snapshot")
))]
mod startup_snapshot {
  use std::path::Path;

  use super::*;
  use deno_ast::MediaType;
  use deno_ast::ParseParams;
  use deno_ast::SourceTextInfo;
  use deno_cache::SqliteBackedCache;
  use deno_core::error::AnyError;
  use deno_core::include_js_files;
  use deno_core::snapshot_util::*;
  use deno_core::Extension;
  use deno_core::ExtensionFileSource;

  fn transpile_ts_for_snapshotting(
    file_source: &ExtensionFileSource,
  ) -> Result<String, AnyError> {
    let media_type = MediaType::from(Path::new(&file_source.specifier));

    let should_transpile = match media_type {
      MediaType::JavaScript => false,
      MediaType::Mjs => false,
      MediaType::TypeScript => true,
      _ => panic!(
        "Unsupported media type for snapshotting {media_type:?} for file {}",
        file_source.specifier
      ),
    };
    let code = file_source.code.load()?;

    if !should_transpile {
      return Ok(code);
    }

    let parsed = deno_ast::parse_module(ParseParams {
      specifier: file_source.specifier.to_string(),
      text_info: SourceTextInfo::from_string(code),
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

    Ok(transpiled_source.text)
  }

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

  impl deno_flash::FlashPermissions for Permissions {
    fn check_net<T: AsRef<str>>(
      &mut self,
      _host: &(T, Option<u16>),
      _api_name: &str,
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

    fn check_read_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }

    fn check_write_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
      unreachable!("snapshotting!")
    }
  }

  fn create_runtime_snapshot(
    snapshot_path: PathBuf,
    maybe_additional_extension: Option<Extension>,
  ) {
    let runtime_extension = Extension::builder_with_deps(
      "runtime",
      &[
        "deno_webidl",
        "deno_console",
        "deno_url",
        "deno_tls",
        "deno_web",
        "deno_fetch",
        "deno_cache",
        "deno_websocket",
        "deno_webstorage",
        "deno_crypto",
        "deno_webgpu",
        "deno_broadcast_channel",
        // FIXME(bartlomieju): this should be reenabled
        // "deno_node",
        "deno_ffi",
        "deno_net",
        "deno_napi",
        "deno_http",
        "deno_flash",
        "deno_io",
        "deno_fs",
      ],
    )
    .esm(include_js_files!(
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
      "98_global_scope.js",
    ))
    .build();

    let mut extensions: Vec<Extension> = vec![
      deno_webidl::init_esm(),
      deno_console::init_esm(),
      deno_url::init_ops_and_esm(),
      deno_tls::init_ops(),
      deno_web::init_ops_and_esm::<Permissions>(
        deno_web::BlobStore::default(),
        Default::default(),
      ),
      deno_fetch::init_ops_and_esm::<Permissions>(Default::default()),
      deno_cache::init_ops_and_esm::<SqliteBackedCache>(None),
      deno_websocket::init_ops_and_esm::<Permissions>(
        "".to_owned(),
        None,
        None,
      ),
      deno_webstorage::init_ops_and_esm(None),
      deno_crypto::init_ops_and_esm(None),
      deno_webgpu::init_ops_and_esm(false),
      deno_broadcast_channel::init_ops_and_esm(
        deno_broadcast_channel::InMemoryBroadcastChannel::default(),
        false, // No --unstable.
      ),
      deno_ffi::init_ops_and_esm::<Permissions>(false),
      deno_net::init_ops_and_esm::<Permissions>(
        None, false, // No --unstable.
        None,
      ),
      deno_napi::init_ops::<Permissions>(),
      deno_http::init_ops_and_esm(),
      deno_io::init_ops_and_esm(Default::default()),
      deno_fs::init_ops_and_esm::<Permissions>(false),
      deno_flash::init_ops_and_esm::<Permissions>(false), // No --unstable
      runtime_extension,
      // FIXME(bartlomieju): these extensions are specified last, because they
      // depend on `runtime`, even though it should be other way around
      deno_node::init_ops_and_esm::<Permissions>(None),
      deno_node::init_polyfill_ops_and_esm(),
    ];

    if let Some(additional_extension) = maybe_additional_extension {
      extensions.push(additional_extension);
    }

    create_snapshot(CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      snapshot_path,
      startup_snapshot: None,
      extensions,
      compression_cb: Some(Box::new(|vec, snapshot_slice| {
        lzzzz::lz4_hc::compress_to_vec(
          snapshot_slice,
          vec,
          lzzzz::lz4_hc::CLEVEL_MAX,
        )
        .expect("snapshot compression failed");
      })),
      snapshot_module_load_cb: Some(Box::new(transpile_ts_for_snapshotting)),
    });
  }

  pub fn build_snapshot(runtime_snapshot_path: PathBuf) {
    #[allow(unused_mut, unused_assignments)]
    let mut maybe_additional_extension = None;

    #[cfg(not(feature = "snapshot_from_snapshot"))]
    {
      use deno_core::ExtensionFileSourceCode;
      maybe_additional_extension = Some(
        Extension::builder_with_deps("runtime_main", vec!["runtime"])
          .esm(vec![ExtensionFileSource {
            specifier: "js/99_main.js".to_string(),
            code: ExtensionFileSourceCode::IncludedInBinary(include_str!(
              "js/99_main.js"
            )),
          }])
          .build(),
      );
    }

    create_runtime_snapshot(runtime_snapshot_path, maybe_additional_extension);
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
  startup_snapshot::build_snapshot(runtime_snapshot_path)
}
