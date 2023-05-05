// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::get_root_cert_store;
use crate::args::CaData;
use crate::args::CacheSetting;
use crate::args::StorageKeyResolver;
use crate::cache::DenoDir;
use crate::file_fetcher::get_source_from_data_url;
use crate::http_util::HttpClient;
use crate::npm::create_npm_fs_resolver;
use crate::npm::CliNpmRegistryApi;
use crate::npm::CliNpmResolver;
use crate::npm::NpmCache;
use crate::npm::NpmResolution;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::v8::construct_v8_flags;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;
use crate::worker::HasNodeSpecifierChecker;
use crate::worker::ModuleLoaderFactory;
use crate::CliGraphResolver;
use deno_core::anyhow::Context;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::v8_set_flags;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use deno_graph::source::Resolver;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use import_map::parse_from_json;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

mod binary;

pub use binary::extract_standalone;
pub use binary::is_standalone_binary;
pub use binary::DenoCompileBinaryWriter;

use self::binary::Metadata;

#[derive(Clone)]
struct EmbeddedModuleLoader {
  eszip: Arc<eszip::EszipV2>,
  maybe_import_map_resolver: Option<Arc<CliGraphResolver>>,
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    // Try to follow redirects when resolving.
    let referrer = match self.eszip.get_module(referrer) {
      Some(eszip::Module { ref specifier, .. }) => {
        ModuleSpecifier::parse(specifier)?
      }
      None => {
        let cwd = std::env::current_dir().context("Unable to get CWD")?;
        deno_core::resolve_url_or_path(referrer, &cwd)?
      }
    };

    self
      .maybe_import_map_resolver
      .as_ref()
      .map(|r| r.resolve(specifier, &referrer))
      .unwrap_or_else(|| {
        deno_core::resolve_import(specifier, referrer.as_str())
          .map_err(|err| err.into())
      })
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let is_data_uri = get_source_from_data_url(module_specifier).ok();
    let module = self
      .eszip
      .get_module(module_specifier.as_str())
      .ok_or_else(|| type_error("Module not found"));
    // TODO(mmastrac): This clone can probably be removed in the future if ModuleSpecifier is no longer a full-fledged URL
    let module_specifier = module_specifier.clone();

    async move {
      if let Some((source, _)) = is_data_uri {
        return Ok(deno_core::ModuleSource::new(
          deno_core::ModuleType::JavaScript,
          source.into(),
          &module_specifier,
        ));
      }

      let module = module?;
      let code = module.source().await.unwrap_or_default();
      let code = std::str::from_utf8(&code)
        .map_err(|_| type_error("Module source is not utf-8"))?
        .to_owned()
        .into();

      Ok(deno_core::ModuleSource::new(
        match module.kind {
          eszip::ModuleKind::JavaScript => ModuleType::JavaScript,
          eszip::ModuleKind::Json => ModuleType::Json,
        },
        code,
        &module_specifier,
      ))
    }
    .boxed_local()
  }
}

struct StandaloneModuleLoaderFactory {
  loader: EmbeddedModuleLoader,
}

impl ModuleLoaderFactory for StandaloneModuleLoaderFactory {
  fn create_for_main(
    &self,
    _root_permissions: PermissionsContainer,
    _dynamic_permissions: PermissionsContainer,
  ) -> Rc<dyn ModuleLoader> {
    Rc::new(self.loader.clone())
  }

  fn create_for_worker(
    &self,
    _root_permissions: PermissionsContainer,
    _dynamic_permissions: PermissionsContainer,
  ) -> Rc<dyn ModuleLoader> {
    Rc::new(self.loader.clone())
  }

  fn create_source_map_getter(
    &self,
  ) -> Option<Box<dyn deno_core::SourceMapGetter>> {
    None
  }
}

struct StandaloneHasNodeSpecifierChecker;

impl HasNodeSpecifierChecker for StandaloneHasNodeSpecifierChecker {
  fn has_node_specifier(&self) -> bool {
    false
  }
}

struct StandaloneRootCertStoreProvider {
  ca_stores: Option<Vec<String>>,
  ca_data: Option<CaData>,
  cell: once_cell::sync::OnceCell<RootCertStore>,
}

impl RootCertStoreProvider for StandaloneRootCertStoreProvider {
  fn get_or_try_init(&self) -> Result<&RootCertStore, AnyError> {
    self.cell.get_or_try_init(|| {
      get_root_cert_store(None, self.ca_stores.clone(), self.ca_data.clone())
        .map_err(|err| err.into())
    })
  }
}

pub async fn run(
  eszip: eszip::EszipV2,
  metadata: Metadata,
) -> Result<(), AnyError> {
  let main_module = &metadata.entrypoint;
  let dir = DenoDir::new(None)?;
  let root_cert_store_provider = Arc::new(StandaloneRootCertStoreProvider {
    ca_stores: metadata.ca_stores,
    ca_data: metadata.ca_data.map(CaData::Bytes),
    cell: Default::default(),
  });
  let progress_bar = ProgressBar::new(ProgressBarStyle::TextOnly);
  let http_client = Arc::new(HttpClient::new(
    Some(root_cert_store_provider.clone()),
    metadata.unsafely_ignore_certificate_errors.clone(),
  ));
  let npm_registry_url = CliNpmRegistryApi::default_url().to_owned();
  let npm_cache = Arc::new(NpmCache::new(
    dir.npm_folder_path(),
    CacheSetting::Use,
    http_client.clone(),
    progress_bar.clone(),
  ));
  let npm_api = Arc::new(CliNpmRegistryApi::new(
    npm_registry_url.clone(),
    npm_cache.clone(),
    http_client.clone(),
    progress_bar.clone(),
  ));
  let fs = Arc::new(deno_fs::RealFs);
  let npm_resolution =
    Arc::new(NpmResolution::from_serialized(npm_api.clone(), None, None));
  let npm_fs_resolver = create_npm_fs_resolver(
    fs.clone(),
    npm_cache,
    &progress_bar,
    npm_registry_url,
    npm_resolution.clone(),
    None,
  );
  let npm_resolver = Arc::new(CliNpmResolver::new(
    npm_resolution.clone(),
    npm_fs_resolver,
    None,
  ));
  let node_resolver =
    Arc::new(NodeResolver::new(fs.clone(), npm_resolver.clone()));
  let module_loader_factory = StandaloneModuleLoaderFactory {
    loader: EmbeddedModuleLoader {
      eszip: Arc::new(eszip),
      maybe_import_map_resolver: metadata.maybe_import_map.map(
        |(base, source)| {
          Arc::new(CliGraphResolver::new(
            None,
            Some(Arc::new(
              parse_from_json(&base, &source).unwrap().import_map,
            )),
            false,
            npm_api.clone(),
            npm_resolution.clone(),
            Default::default(),
          ))
        },
      ),
    },
  };

  let worker_factory = CliMainWorkerFactory::new(
    StorageKeyResolver::empty(),
    npm_resolver.clone(),
    node_resolver,
    Box::new(StandaloneHasNodeSpecifierChecker),
    BlobStore::default(),
    Box::new(module_loader_factory),
    root_cert_store_provider,
    fs,
    None,
    CliMainWorkerOptions {
      argv: metadata.argv,
      debug: false,
      coverage_dir: None,
      enable_testing_features: false,
      has_node_modules_dir: false,
      inspect_brk: false,
      inspect_wait: false,
      is_inspecting: false,
      is_npm_main: false,
      location: metadata.location,
      // todo(dsherret): support a binary command being compiled
      maybe_binary_npm_command_name: None,
      origin_data_folder_path: None,
      seed: metadata.seed,
      unsafely_ignore_certificate_errors: metadata
        .unsafely_ignore_certificate_errors,
      unstable: metadata.unstable,
    },
  );

  v8_set_flags(construct_v8_flags(&metadata.v8_flags, vec![]));

  let permissions = PermissionsContainer::new(Permissions::from_options(
    &metadata.permissions,
  )?);
  let mut worker = worker_factory
    .create_main_worker(main_module.clone(), permissions)
    .await?;

  let exit_code = worker.run().await?;
  std::process::exit(exit_code)
}
