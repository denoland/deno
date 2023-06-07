// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::get_root_cert_store;
use crate::args::npm_pkg_req_ref_to_binary_command;
use crate::args::CaData;
use crate::args::CacheSetting;
use crate::args::PackageJsonDepsProvider;
use crate::args::StorageKeyResolver;
use crate::cache::Caches;
use crate::cache::DenoDirProvider;
use crate::cache::NodeAnalysisCache;
use crate::file_fetcher::get_source_from_data_url;
use crate::http_util::HttpClient;
use crate::module_loader::CjsResolutionStore;
use crate::module_loader::NpmModuleLoader;
use crate::node::CliCjsEsmCodeAnalyzer;
use crate::npm::create_npm_fs_resolver;
use crate::npm::CliNpmRegistryApi;
use crate::npm::CliNpmResolver;
use crate::npm::NpmCache;
use crate::npm::NpmResolution;
use crate::resolver::MappedSpecifierResolver;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::v8::construct_v8_flags;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;
use crate::worker::HasNodeSpecifierChecker;
use crate::worker::ModuleLoaderFactory;
use deno_ast::MediaType;
use deno_core::anyhow::Context;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::v8_set_flags;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::analyze::NodeCodeTranslator;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::WorkerLogLevel;
use deno_semver::npm::NpmPackageReqReference;
use import_map::parse_from_json;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

mod binary;
mod file_system;
mod virtual_fs;

pub use binary::extract_standalone;
pub use binary::is_standalone_binary;
pub use binary::DenoCompileBinaryWriter;

use self::binary::load_npm_vfs;
use self::binary::Metadata;
use self::file_system::DenoCompileFileSystem;

struct SharedModuleLoaderState {
  eszip: eszip::EszipV2,
  mapped_specifier_resolver: MappedSpecifierResolver,
  npm_module_loader: Arc<NpmModuleLoader>,
}

#[derive(Clone)]
struct EmbeddedModuleLoader {
  shared: Arc<SharedModuleLoaderState>,
  root_permissions: PermissionsContainer,
  dynamic_permissions: PermissionsContainer,
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    // Try to follow redirects when resolving.
    let referrer = match self.shared.eszip.get_module(referrer) {
      Some(eszip::Module { ref specifier, .. }) => {
        ModuleSpecifier::parse(specifier)?
      }
      None => {
        let cwd = std::env::current_dir().context("Unable to get CWD")?;
        deno_core::resolve_url_or_path(referrer, &cwd)?
      }
    };

    let permissions = if matches!(kind, ResolutionKind::DynamicImport) {
      &self.dynamic_permissions
    } else {
      &self.root_permissions
    };

    if let Some(result) = self
      .shared
      .npm_module_loader
      .resolve_if_in_npm_package(specifier, &referrer, permissions)
    {
      return result;
    }

    let maybe_mapped = self
      .shared
      .mapped_specifier_resolver
      .resolve(specifier, &referrer)?
      .into_specifier();

    // npm specifier
    let specifier_text = maybe_mapped
      .as_ref()
      .map(|r| r.as_str())
      .unwrap_or(specifier);
    if let Ok(reference) = NpmPackageReqReference::from_str(specifier_text) {
      return self
        .shared
        .npm_module_loader
        .resolve_req_reference(&reference, permissions);
    }

    match maybe_mapped {
      Some(resolved) => Ok(resolved),
      None => deno_core::resolve_import(specifier, referrer.as_str())
        .map_err(|err| err.into()),
    }
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let is_data_uri = get_source_from_data_url(module_specifier).ok();
    let permissions = if is_dynamic {
      &self.dynamic_permissions
    } else {
      &self.root_permissions
    };

    if let Some(result) =
      self.shared.npm_module_loader.load_sync_if_in_npm_package(
        module_specifier,
        maybe_referrer,
        permissions,
      )
    {
      return match result {
        Ok(code_source) => Box::pin(deno_core::futures::future::ready(Ok(
          deno_core::ModuleSource::new_with_redirect(
            match code_source.media_type {
              MediaType::Json => ModuleType::Json,
              _ => ModuleType::JavaScript,
            },
            code_source.code,
            module_specifier,
            &code_source.found_url,
          ),
        ))),
        Err(err) => Box::pin(deno_core::futures::future::ready(Err(err))),
      };
    }

    let module = self
      .shared
      .eszip
      .get_module(module_specifier.as_str())
      .ok_or_else(|| {
        type_error(format!("Module not found: {}", module_specifier))
      });
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
          eszip::ModuleKind::Jsonc => {
            return Err(type_error("jsonc modules not supported"))
          }
        },
        code,
        &module_specifier,
      ))
    }
    .boxed_local()
  }
}

struct StandaloneModuleLoaderFactory {
  shared: Arc<SharedModuleLoaderState>,
}

impl ModuleLoaderFactory for StandaloneModuleLoaderFactory {
  fn create_for_main(
    &self,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Rc<dyn ModuleLoader> {
    Rc::new(EmbeddedModuleLoader {
      shared: self.shared.clone(),
      root_permissions,
      dynamic_permissions,
    })
  }

  fn create_for_worker(
    &self,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Rc<dyn ModuleLoader> {
    Rc::new(EmbeddedModuleLoader {
      shared: self.shared.clone(),
      root_permissions,
      dynamic_permissions,
    })
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
  let current_exe_path = std::env::current_exe().unwrap();
  let current_exe_name =
    current_exe_path.file_name().unwrap().to_string_lossy();
  let deno_dir_provider = Arc::new(DenoDirProvider::new(None));
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
  // use a dummy npm registry url
  let npm_registry_url = ModuleSpecifier::parse("https://localhost/").unwrap();
  let root_path = std::env::temp_dir()
    .join(format!("deno-compile-{}", current_exe_name))
    .join("node_modules");

  let npm_cache = Arc::new(NpmCache::new(
    root_path.clone(),
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
  let (fs, vfs_root, node_modules_path, snapshot) = if let Some(snapshot) =
    metadata.npm_snapshot
  {
    let vfs_root_dir_path = if metadata.node_modules_dir {
      root_path
    } else {
      npm_cache.registry_folder(&npm_registry_url)
    };
    let vfs = load_npm_vfs(vfs_root_dir_path.clone())
      .context("Failed to load npm vfs.")?;
    let node_modules_path = if metadata.node_modules_dir {
      Some(vfs.root().to_path_buf())
    } else {
      None
    };
    (
      Arc::new(DenoCompileFileSystem::new(vfs)) as Arc<dyn deno_fs::FileSystem>,
      Some(vfs_root_dir_path),
      node_modules_path,
      Some(snapshot.into_valid()?),
    )
  } else {
    (
      Arc::new(deno_fs::RealFs) as Arc<dyn deno_fs::FileSystem>,
      None,
      None,
      None,
    )
  };
  let npm_resolution = Arc::new(NpmResolution::from_serialized(
    npm_api.clone(),
    snapshot,
    None,
  ));
  let has_node_modules_dir = node_modules_path.is_some();
  let npm_fs_resolver = create_npm_fs_resolver(
    fs.clone(),
    npm_cache,
    &progress_bar,
    npm_registry_url,
    npm_resolution.clone(),
    node_modules_path,
    NpmSystemInfo::default(),
  );
  let npm_resolver = Arc::new(CliNpmResolver::new(
    fs.clone(),
    npm_resolution.clone(),
    npm_fs_resolver,
    None,
  ));
  let node_resolver =
    Arc::new(NodeResolver::new(fs.clone(), npm_resolver.clone()));
  let cjs_resolutions = Arc::new(CjsResolutionStore::default());
  let cache_db = Caches::new(deno_dir_provider.clone());
  let node_analysis_cache = NodeAnalysisCache::new(cache_db.node_analysis_db());
  let cjs_esm_code_analyzer = CliCjsEsmCodeAnalyzer::new(node_analysis_cache);
  let node_code_translator = Arc::new(NodeCodeTranslator::new(
    cjs_esm_code_analyzer,
    fs.clone(),
    node_resolver.clone(),
    npm_resolver.clone(),
  ));
  let package_json_deps_provider = Arc::new(PackageJsonDepsProvider::new(
    metadata
      .package_json_deps
      .map(|serialized| serialized.into_deps()),
  ));
  let maybe_import_map = metadata.maybe_import_map.map(|(base, source)| {
    Arc::new(parse_from_json(&base, &source).unwrap().import_map)
  });
  let module_loader_factory = StandaloneModuleLoaderFactory {
    shared: Arc::new(SharedModuleLoaderState {
      eszip,
      mapped_specifier_resolver: MappedSpecifierResolver::new(
        maybe_import_map.clone(),
        package_json_deps_provider.clone(),
      ),
      npm_module_loader: Arc::new(NpmModuleLoader::new(
        cjs_resolutions,
        node_code_translator,
        fs.clone(),
        node_resolver.clone(),
      )),
    }),
  };

  let permissions = {
    let mut permissions = metadata.permissions;
    // if running with an npm vfs, grant read access to it
    if let Some(vfs_root) = vfs_root {
      match &mut permissions.allow_read {
        Some(vec) if vec.is_empty() => {
          // do nothing, already granted
        }
        Some(vec) => {
          vec.push(vfs_root);
        }
        None => {
          permissions.allow_read = Some(vec![vfs_root]);
        }
      }
    }

    PermissionsContainer::new(Permissions::from_options(&permissions)?)
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
    None,
    CliMainWorkerOptions {
      argv: metadata.argv,
      log_level: WorkerLogLevel::Info,
      coverage_dir: None,
      enable_testing_features: false,
      has_node_modules_dir,
      inspect_brk: false,
      inspect_wait: false,
      is_inspecting: false,
      is_npm_main: main_module.scheme() == "npm",
      location: metadata.location,
      maybe_binary_npm_command_name: NpmPackageReqReference::from_specifier(
        main_module,
      )
      .ok()
      .map(|req_ref| npm_pkg_req_ref_to_binary_command(&req_ref)),
      origin_data_folder_path: None,
      seed: metadata.seed,
      unsafely_ignore_certificate_errors: metadata
        .unsafely_ignore_certificate_errors,
      unstable: metadata.unstable,
    },
  );

  v8_set_flags(construct_v8_flags(&[], &metadata.v8_flags, vec![]));

  let mut worker = worker_factory
    .create_main_worker(main_module.clone(), permissions)
    .await?;

  let exit_code = worker.run().await?;
  std::process::exit(exit_code)
}
