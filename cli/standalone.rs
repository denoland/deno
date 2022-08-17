// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::colors;
use crate::file_fetcher::get_source_from_data_url;
use crate::fmt_errors::format_js_error;
use crate::ops;
use crate::proc_state::ProcState;
use crate::version;
use crate::ImportMapResolver;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::located_script_name;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_core::v8_set_flags;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_graph::source::Resolver;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_tls::rustls_pemfile;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use import_map::parse_from_json;
use log::Level;
use std::env::current_exe;
use std::io::BufReader;
use std::io::Cursor;
use std::io::SeekFrom;
use std::iter::once;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[derive(Deserialize, Serialize)]
pub struct Metadata {
  pub argv: Vec<String>,
  pub unstable: bool,
  pub seed: Option<u64>,
  pub permissions: PermissionsOptions,
  pub location: Option<Url>,
  pub v8_flags: Vec<String>,
  pub log_level: Option<Level>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<Vec<u8>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub maybe_import_map: Option<(Url, String)>,
  pub entrypoint: ModuleSpecifier,
}

pub const MAGIC_TRAILER: &[u8; 8] = b"d3n0l4nd";

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by checking for the magic trailer string `D3N0` at EOF-12.
/// The magic trailer is followed by:
/// - a u64 pointer to the JS bundle embedded in the binary
/// - a u64 pointer to JSON metadata (serialized flags) embedded in the binary
/// These are dereferenced, and the bundle is executed under the configuration
/// specified by the metadata. If no magic trailer is present, this function
/// exits with `Ok(None)`.
pub async fn extract_standalone(
  args: Vec<String>,
) -> Result<Option<(Metadata, eszip::EszipV2)>, AnyError> {
  let current_exe_path = current_exe()?;

  let file = tokio::fs::File::open(current_exe_path).await?;

  let mut bufreader = tokio::io::BufReader::new(file);

  let trailer_pos = bufreader.seek(SeekFrom::End(-24)).await?;
  let mut trailer = [0; 24];
  bufreader.read_exact(&mut trailer).await?;
  let (magic_trailer, rest) = trailer.split_at(8);
  if magic_trailer != MAGIC_TRAILER {
    return Ok(None);
  }

  let (eszip_archive_pos, rest) = rest.split_at(8);
  let metadata_pos = rest;
  let eszip_archive_pos = u64_from_bytes(eszip_archive_pos)?;
  let metadata_pos = u64_from_bytes(metadata_pos)?;
  let metadata_len = trailer_pos - metadata_pos;

  bufreader.seek(SeekFrom::Start(eszip_archive_pos)).await?;

  let (eszip, loader) = eszip::EszipV2::parse(bufreader)
    .await
    .context("Failed to parse eszip header")?;

  let mut bufreader = loader.await.context("Failed to parse eszip archive")?;

  bufreader.seek(SeekFrom::Start(metadata_pos)).await?;

  let mut metadata = String::new();

  bufreader
    .take(metadata_len)
    .read_to_string(&mut metadata)
    .await
    .context("Failed to read metadata from the current executable")?;

  let mut metadata: Metadata = serde_json::from_str(&metadata).unwrap();
  metadata.argv.append(&mut args[1..].to_vec());

  Ok(Some((metadata, eszip)))
}

fn u64_from_bytes(arr: &[u8]) -> Result<u64, AnyError> {
  let fixed_arr: &[u8; 8] = arr
    .try_into()
    .context("Failed to convert the buffer into a fixed-size array")?;
  Ok(u64::from_be_bytes(*fixed_arr))
}

struct EmbeddedModuleLoader {
  eszip: eszip::EszipV2,
  maybe_import_map_resolver: Option<ImportMapResolver>,
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    // Try to follow redirects when resolving.
    let referrer = match self.eszip.get_module(referrer) {
      Some(eszip::Module { ref specifier, .. }) => {
        deno_core::resolve_url_or_path(specifier)?
      }
      None => deno_core::resolve_url_or_path(referrer)?,
    };

    self.maybe_import_map_resolver.as_ref().map_or_else(
      || {
        deno_core::resolve_import(specifier, referrer.as_str())
          .map_err(|err| err.into())
      },
      |r| r.resolve(specifier, &referrer).to_result(),
    )
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.clone();

    let is_data_uri = get_source_from_data_url(&module_specifier).ok();
    let module = self
      .eszip
      .get_module(module_specifier.as_str())
      .ok_or_else(|| type_error("Module not found"));

    async move {
      if let Some((source, _)) = is_data_uri {
        return Ok(deno_core::ModuleSource {
          code: source.into_bytes().into_boxed_slice(),
          module_type: deno_core::ModuleType::JavaScript,
          module_url_specified: module_specifier.to_string(),
          module_url_found: module_specifier.to_string(),
        });
      }

      let module = module?;
      let code = module.source().await;
      let code = std::str::from_utf8(&code)
        .map_err(|_| type_error("Module source is not utf-8"))?
        .to_owned();

      Ok(deno_core::ModuleSource {
        code: code.into_bytes().into_boxed_slice(),
        module_type: match module.kind {
          eszip::ModuleKind::JavaScript => deno_core::ModuleType::JavaScript,
          eszip::ModuleKind::Json => deno_core::ModuleType::Json,
        },
        module_url_specified: module_specifier.to_string(),
        module_url_found: module_specifier.to_string(),
      })
    }
    .boxed_local()
  }
}

fn metadata_to_flags(metadata: &Metadata) -> Flags {
  let permissions = metadata.permissions.clone();
  Flags {
    argv: metadata.argv.clone(),
    unstable: metadata.unstable,
    seed: metadata.seed,
    location: metadata.location.clone(),
    allow_env: permissions.allow_env,
    allow_hrtime: permissions.allow_hrtime,
    allow_net: permissions.allow_net,
    allow_ffi: permissions.allow_ffi,
    allow_read: permissions.allow_read,
    allow_run: permissions.allow_run,
    allow_write: permissions.allow_write,
    v8_flags: metadata.v8_flags.clone(),
    log_level: metadata.log_level,
    ca_stores: metadata.ca_stores.clone(),
    ..Default::default()
  }
}

pub async fn run(
  eszip: eszip::EszipV2,
  metadata: Metadata,
) -> Result<(), AnyError> {
  let flags = metadata_to_flags(&metadata);
  let main_module = &metadata.entrypoint;
  let ps = ProcState::build(flags).await?;
  let permissions = Permissions::from_options(&metadata.permissions)?;
  let blob_store = BlobStore::default();
  let broadcast_channel = InMemoryBroadcastChannel::default();
  let module_loader = Rc::new(EmbeddedModuleLoader {
    eszip,
    maybe_import_map_resolver: metadata.maybe_import_map.map(
      |(base, source)| {
        ImportMapResolver::new(Arc::new(
          parse_from_json(&base, &source).unwrap().import_map,
        ))
      },
    ),
  });
  let create_web_worker_cb = Arc::new(|_| {
    todo!("Worker are currently not supported in standalone binaries");
  });
  let web_worker_preload_module_cb = Arc::new(|_| {
    todo!("Worker are currently not supported in standalone binaries");
  });

  // Keep in sync with `main.rs`.
  v8_set_flags(
    once("UNUSED_BUT_NECESSARY_ARG0".to_owned())
      .chain(metadata.v8_flags.iter().cloned())
      .collect::<Vec<_>>(),
  );

  let mut root_cert_store = ps.root_cert_store.clone();

  if let Some(cert) = metadata.ca_data {
    let reader = &mut BufReader::new(Cursor::new(cert));
    match rustls_pemfile::certs(reader) {
      Ok(certs) => {
        root_cert_store.add_parsable_certificates(&certs);
      }
      Err(e) => {
        return Err(anyhow!(
          "Unable to add pem file to certificate store: {}",
          e
        ));
      }
    }
  }

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      args: metadata.argv,
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      debug_flag: metadata.log_level.map_or(false, |l| l == Level::Debug),
      enable_testing_features: false,
      location: metadata.location,
      no_color: !colors::use_color(),
      is_tty: colors::is_tty(),
      runtime_version: version::deno(),
      ts_version: version::TYPESCRIPT.to_string(),
      unstable: metadata.unstable,
      user_agent: version::get_user_agent(),
    },
    extensions: ops::cli_exts(ps.clone()),
    unsafely_ignore_certificate_errors: metadata
      .unsafely_ignore_certificate_errors,
    root_cert_store: Some(root_cert_store),
    seed: metadata.seed,
    source_map_getter: None,
    format_js_error_fn: Some(Arc::new(format_js_error)),
    create_web_worker_cb,
    web_worker_preload_module_cb,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    module_loader,
    get_error_class_fn: Some(&get_error_class_name),
    origin_storage_dir: None,
    blob_store,
    broadcast_channel,
    shared_array_buffer_store: None,
    compiled_wasm_module_store: None,
    stdio: Default::default(),
  };
  let mut worker = MainWorker::bootstrap_from_options(
    main_module.clone(),
    permissions,
    options,
  );
  worker.execute_main_module(main_module).await?;
  worker.dispatch_load_event(&located_script_name!())?;

  loop {
    worker.run_event_loop(false).await?;
    if !worker.dispatch_beforeunload_event(&located_script_name!())? {
      break;
    }
  }

  worker.dispatch_unload_event(&located_script_name!())?;
  std::process::exit(0);
}

fn get_error_class_name(e: &AnyError) -> &'static str {
  deno_runtime::errors::get_error_class_name(e).unwrap_or_else(|| {
    panic!(
      "Error '{}' contains boxed error of unsupported type:{}",
      e,
      e.chain()
        .map(|e| format!("\n  {:?}", e))
        .collect::<String>()
    );
  })
}
