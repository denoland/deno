// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Allow unused code warnings because we share
// code between the two bin targets.
#![allow(dead_code)]
#![allow(unused_imports)]

use deno_ast::MediaType;
use deno_cache_dir::npm::NpmCacheDir;
use deno_config::workspace::MappedResolution;
use deno_config::workspace::MappedResolutionError;
use deno_config::workspace::ResolverWorkspaceJsrPackage;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::v8_set_flags;
use deno_core::FeatureChecker;
use deno_core::ModuleLoader;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_package_json::PackageJsonDepValue;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::WorkerLogLevel;
use deno_semver::npm::NpmPackageReqReference;
use eszip::EszipRelativeFileBaseUrl;
use import_map::parse_from_json;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::NodeResolutionMode;
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

use crate::args::create_default_npmrc;
use crate::args::get_root_cert_store;
use crate::args::npm_pkg_req_ref_to_binary_command;
use crate::args::CaData;
use crate::args::CacheSetting;
use crate::args::NpmInstallDepsProvider;
use crate::args::StorageKeyResolver;
use crate::cache::Caches;
use crate::cache::DenoDirProvider;
use crate::cache::NodeAnalysisCache;
use crate::cache::RealDenoCacheEnv;
use crate::http_util::HttpClientProvider;
use crate::node::CliCjsCodeAnalyzer;
use crate::npm::create_cli_npm_resolver;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedCreateOptions;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::resolver::CjsResolutionStore;
use crate::resolver::CliDenoResolverFs;
use crate::resolver::CliNodeResolver;
use crate::resolver::NpmModuleLoader;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::v8::construct_v8_flags;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;
use crate::worker::ModuleLoaderAndSourceMapGetter;
use crate::worker::ModuleLoaderFactory;

pub mod binary;
mod file_system;
mod virtual_fs;

pub use binary::extract_standalone;
pub use binary::is_standalone_binary;
pub use binary::DenoCompileBinaryWriter;

use self::binary::load_npm_vfs;
use self::binary::Metadata;
use self::file_system::DenoCompileFileSystem;

struct WorkspaceEszipModule {
  specifier: ModuleSpecifier,
  inner: eszip::Module,
}

struct WorkspaceEszip {
  eszip: eszip::EszipV2,
  root_dir_url: Arc<ModuleSpecifier>,
}

impl WorkspaceEszip {
  pub fn get_module(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<WorkspaceEszipModule> {
    if specifier.scheme() == "file" {
      let specifier_key = EszipRelativeFileBaseUrl::new(&self.root_dir_url)
        .specifier_key(specifier);
      let module = self.eszip.get_module(&specifier_key)?;
      let specifier = self.root_dir_url.join(&module.specifier).unwrap();
      Some(WorkspaceEszipModule {
        specifier,
        inner: module,
      })
    } else {
      let module = self.eszip.get_module(specifier.as_str())?;
      Some(WorkspaceEszipModule {
        specifier: ModuleSpecifier::parse(&module.specifier).unwrap(),
        inner: module,
      })
    }
  }
}

struct SharedModuleLoaderState {
  eszip: WorkspaceEszip,
  workspace_resolver: WorkspaceResolver,
  node_resolver: Arc<CliNodeResolver>,
  npm_module_loader: Arc<NpmModuleLoader>,
}

#[derive(Clone)]
struct EmbeddedModuleLoader {
  shared: Arc<SharedModuleLoaderState>,
}

pub const MODULE_NOT_FOUND: &str = "Module not found";
pub const UNSUPPORTED_SCHEME: &str = "Unsupported scheme";

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    raw_specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    let referrer = if referrer == "." {
      if kind != ResolutionKind::MainModule {
        return Err(generic_error(format!(
          "Expected to resolve main module, got {:?} instead.",
          kind
        )));
      }
      let current_dir = std::env::current_dir().unwrap();
      deno_core::resolve_path(".", &current_dir)?
    } else {
      ModuleSpecifier::parse(referrer).map_err(|err| {
        type_error(format!("Referrer uses invalid specifier: {}", err))
      })?
    };

    if self.shared.node_resolver.in_npm_package(&referrer) {
      return Ok(
        self
          .shared
          .node_resolver
          .resolve(raw_specifier, &referrer, NodeResolutionMode::Execution)?
          .into_url(),
      );
    }

    let mapped_resolution = self
      .shared
      .workspace_resolver
      .resolve(raw_specifier, &referrer);

    match mapped_resolution {
      Ok(MappedResolution::WorkspaceJsrPackage { specifier, .. }) => {
        Ok(specifier)
      }
      Ok(MappedResolution::WorkspaceNpmPackage {
        target_pkg_json: pkg_json,
        sub_path,
        ..
      }) => Ok(
        self
          .shared
          .node_resolver
          .resolve_package_sub_path_from_deno_module(
            pkg_json.dir_path(),
            sub_path.as_deref(),
            Some(&referrer),
            NodeResolutionMode::Execution,
          )?
          .into_url(),
      ),
      Ok(MappedResolution::PackageJson {
        dep_result,
        sub_path,
        alias,
        ..
      }) => match dep_result.as_ref().map_err(|e| AnyError::from(e.clone()))? {
        PackageJsonDepValue::Req(req) => self
          .shared
          .node_resolver
          .resolve_req_with_sub_path(
            req,
            sub_path.as_deref(),
            &referrer,
            NodeResolutionMode::Execution,
          )
          .map(|res| res.into_url()),
        PackageJsonDepValue::Workspace(version_req) => {
          let pkg_folder = self
            .shared
            .workspace_resolver
            .resolve_workspace_pkg_json_folder_for_pkg_json_dep(
              alias,
              version_req,
            )?;
          Ok(
            self
              .shared
              .node_resolver
              .resolve_package_sub_path_from_deno_module(
                pkg_folder,
                sub_path.as_deref(),
                Some(&referrer),
                NodeResolutionMode::Execution,
              )?
              .into_url(),
          )
        }
      },
      Ok(MappedResolution::Normal { specifier, .. })
      | Ok(MappedResolution::ImportMap { specifier, .. }) => {
        if let Ok(reference) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          return self
            .shared
            .node_resolver
            .resolve_req_reference(
              &reference,
              &referrer,
              NodeResolutionMode::Execution,
            )
            .map(|res| res.into_url());
        }

        if specifier.scheme() == "jsr" {
          if let Some(module) = self.shared.eszip.get_module(&specifier) {
            return Ok(module.specifier);
          }
        }

        Ok(
          self
            .shared
            .node_resolver
            .handle_if_in_node_modules(&specifier)?
            .unwrap_or(specifier),
        )
      }
      Err(err)
        if err.is_unmapped_bare_specifier() && referrer.scheme() == "file" =>
      {
        let maybe_res = self.shared.node_resolver.resolve_if_for_npm_pkg(
          raw_specifier,
          &referrer,
          NodeResolutionMode::Execution,
        )?;
        if let Some(res) = maybe_res {
          return Ok(res.into_url());
        }
        Err(err.into())
      }
      Err(err) => Err(err.into()),
    }
  }

  fn get_host_defined_options<'s>(
    &self,
    scope: &mut deno_core::v8::HandleScope<'s>,
    name: &str,
  ) -> Option<deno_core::v8::Local<'s, deno_core::v8::Data>> {
    let name = deno_core::ModuleSpecifier::parse(name).ok()?;
    if self.shared.node_resolver.in_npm_package(&name) {
      Some(create_host_defined_options(scope))
    } else {
      None
    }
  }

  fn load(
    &self,
    original_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    _is_dynamic: bool,
    _requested_module_type: RequestedModuleType,
  ) -> deno_core::ModuleLoadResponse {
    if original_specifier.scheme() == "data" {
      let data_url_text =
        match deno_graph::source::RawDataUrl::parse(original_specifier)
          .and_then(|url| url.decode())
        {
          Ok(response) => response,
          Err(err) => {
            return deno_core::ModuleLoadResponse::Sync(Err(type_error(
              format!("{:#}", err),
            )));
          }
        };
      return deno_core::ModuleLoadResponse::Sync(Ok(
        deno_core::ModuleSource::new(
          deno_core::ModuleType::JavaScript,
          ModuleSourceCode::String(data_url_text.into()),
          original_specifier,
          None,
        ),
      ));
    }

    if self.shared.node_resolver.in_npm_package(original_specifier) {
      let npm_module_loader = self.shared.npm_module_loader.clone();
      let original_specifier = original_specifier.clone();
      let maybe_referrer = maybe_referrer.cloned();
      return deno_core::ModuleLoadResponse::Async(
        async move {
          let code_source = npm_module_loader
            .load(&original_specifier, maybe_referrer.as_ref())
            .await?;
          Ok(deno_core::ModuleSource::new_with_redirect(
            match code_source.media_type {
              MediaType::Json => ModuleType::Json,
              _ => ModuleType::JavaScript,
            },
            code_source.code,
            &original_specifier,
            &code_source.found_url,
            None,
          ))
        }
        .boxed_local(),
      );
    }

    let Some(module) = self.shared.eszip.get_module(original_specifier) else {
      return deno_core::ModuleLoadResponse::Sync(Err(type_error(format!(
        "{MODULE_NOT_FOUND}: {}",
        original_specifier
      ))));
    };
    let original_specifier = original_specifier.clone();

    deno_core::ModuleLoadResponse::Async(
      async move {
        let code = module.inner.source().await.ok_or_else(|| {
          type_error(format!("Module not found: {}", original_specifier))
        })?;
        let code = arc_u8_to_arc_str(code)
          .map_err(|_| type_error("Module source is not utf-8"))?;
        Ok(deno_core::ModuleSource::new_with_redirect(
          match module.inner.kind {
            eszip::ModuleKind::JavaScript => ModuleType::JavaScript,
            eszip::ModuleKind::Json => ModuleType::Json,
            eszip::ModuleKind::Jsonc => {
              return Err(type_error("jsonc modules not supported"))
            }
            eszip::ModuleKind::OpaqueData => {
              unreachable!();
            }
          },
          ModuleSourceCode::String(code.into()),
          &original_specifier,
          &module.specifier,
          None,
        ))
      }
      .boxed_local(),
    )
  }
}

fn arc_u8_to_arc_str(
  arc_u8: Arc<[u8]>,
) -> Result<Arc<str>, std::str::Utf8Error> {
  // Check that the string is valid UTF-8.
  std::str::from_utf8(&arc_u8)?;
  // SAFETY: the string is valid UTF-8, and the layout Arc<[u8]> is the same as
  // Arc<str>. This is proven by the From<Arc<str>> impl for Arc<[u8]> from the
  // standard library.
  Ok(unsafe {
    std::mem::transmute::<std::sync::Arc<[u8]>, std::sync::Arc<str>>(arc_u8)
  })
}

struct StandaloneModuleLoaderFactory {
  shared: Arc<SharedModuleLoaderState>,
}

impl ModuleLoaderFactory for StandaloneModuleLoaderFactory {
  fn create_for_main(
    &self,
    _root_permissions: PermissionsContainer,
  ) -> ModuleLoaderAndSourceMapGetter {
    ModuleLoaderAndSourceMapGetter {
      module_loader: Rc::new(EmbeddedModuleLoader {
        shared: self.shared.clone(),
      }),
    }
  }

  fn create_for_worker(
    &self,
    _parent_permissions: PermissionsContainer,
    _permissions: PermissionsContainer,
  ) -> ModuleLoaderAndSourceMapGetter {
    ModuleLoaderAndSourceMapGetter {
      module_loader: Rc::new(EmbeddedModuleLoader {
        shared: self.shared.clone(),
      }),
    }
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
  mut eszip: eszip::EszipV2,
  metadata: Metadata,
) -> Result<i32, AnyError> {
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
  let http_client_provider = Arc::new(HttpClientProvider::new(
    Some(root_cert_store_provider.clone()),
    metadata.unsafely_ignore_certificate_errors.clone(),
  ));
  // use a dummy npm registry url
  let npm_registry_url = ModuleSpecifier::parse("https://localhost/").unwrap();
  let root_path =
    std::env::temp_dir().join(format!("deno-compile-{}", current_exe_name));
  let root_dir_url =
    Arc::new(ModuleSpecifier::from_directory_path(&root_path).unwrap());
  let main_module = root_dir_url.join(&metadata.entrypoint_key).unwrap();
  let root_node_modules_path = root_path.join("node_modules");
  let npm_cache_dir = NpmCacheDir::new(
    &RealDenoCacheEnv,
    root_node_modules_path.clone(),
    vec![npm_registry_url.clone()],
  );
  let npm_global_cache_dir = npm_cache_dir.get_cache_location();
  let cache_setting = CacheSetting::Only;
  let (fs, npm_resolver, maybe_vfs_root) = match metadata.node_modules {
    Some(binary::NodeModules::Managed { node_modules_dir }) => {
      // this will always have a snapshot
      let snapshot = eszip.take_npm_snapshot().unwrap();
      let vfs_root_dir_path = if node_modules_dir.is_some() {
        root_path.clone()
      } else {
        npm_cache_dir.root_dir().to_owned()
      };
      let vfs = load_npm_vfs(vfs_root_dir_path.clone())
        .context("Failed to load npm vfs.")?;
      let maybe_node_modules_path = node_modules_dir
        .map(|node_modules_dir| vfs_root_dir_path.join(node_modules_dir));
      let fs = Arc::new(DenoCompileFileSystem::new(vfs))
        as Arc<dyn deno_fs::FileSystem>;
      let npm_resolver =
        create_cli_npm_resolver(CliNpmResolverCreateOptions::Managed(
          CliNpmResolverManagedCreateOptions {
            snapshot: CliNpmResolverManagedSnapshotOption::Specified(Some(
              snapshot,
            )),
            maybe_lockfile: None,
            fs: fs.clone(),
            http_client_provider: http_client_provider.clone(),
            npm_global_cache_dir,
            cache_setting,
            text_only_progress_bar: progress_bar,
            maybe_node_modules_path,
            npm_system_info: Default::default(),
            npm_install_deps_provider: Arc::new(
              // this is only used for installing packages, which isn't necessary with deno compile
              NpmInstallDepsProvider::empty(),
            ),
            // create an npmrc that uses the fake npm_registry_url to resolve packages
            npmrc: Arc::new(ResolvedNpmRc {
              default_config: deno_npm::npm_rc::RegistryConfigWithUrl {
                registry_url: npm_registry_url.clone(),
                config: Default::default(),
              },
              scopes: Default::default(),
              registry_configs: Default::default(),
            }),
            lifecycle_scripts: Default::default(),
          },
        ))
        .await?;
      (fs, npm_resolver, Some(vfs_root_dir_path))
    }
    Some(binary::NodeModules::Byonm {
      root_node_modules_dir,
    }) => {
      let vfs_root_dir_path = root_path.clone();
      let vfs = load_npm_vfs(vfs_root_dir_path.clone())
        .context("Failed to load vfs.")?;
      let root_node_modules_dir =
        root_node_modules_dir.map(|p| vfs.root().join(p));
      let fs = Arc::new(DenoCompileFileSystem::new(vfs))
        as Arc<dyn deno_fs::FileSystem>;
      let npm_resolver = create_cli_npm_resolver(
        CliNpmResolverCreateOptions::Byonm(CliByonmNpmResolverCreateOptions {
          fs: CliDenoResolverFs(fs.clone()),
          root_node_modules_dir,
        }),
      )
      .await?;
      (fs, npm_resolver, Some(vfs_root_dir_path))
    }
    None => {
      let fs = Arc::new(deno_fs::RealFs) as Arc<dyn deno_fs::FileSystem>;
      let npm_resolver =
        create_cli_npm_resolver(CliNpmResolverCreateOptions::Managed(
          CliNpmResolverManagedCreateOptions {
            snapshot: CliNpmResolverManagedSnapshotOption::Specified(None),
            maybe_lockfile: None,
            fs: fs.clone(),
            http_client_provider: http_client_provider.clone(),
            npm_global_cache_dir,
            cache_setting,
            text_only_progress_bar: progress_bar,
            maybe_node_modules_path: None,
            npm_system_info: Default::default(),
            npm_install_deps_provider: Arc::new(
              // this is only used for installing packages, which isn't necessary with deno compile
              NpmInstallDepsProvider::empty(),
            ),
            // Packages from different registries are already inlined in the ESZip,
            // so no need to create actual `.npmrc` configuration.
            npmrc: create_default_npmrc(),
            lifecycle_scripts: Default::default(),
          },
        ))
        .await?;
      (fs, npm_resolver, None)
    }
  };

  let has_node_modules_dir = npm_resolver.root_node_modules_path().is_some();
  let node_resolver = Arc::new(NodeResolver::new(
    deno_runtime::deno_node::DenoFsNodeResolverEnv::new(fs.clone()),
    npm_resolver.clone().into_npm_resolver(),
  ));
  let cjs_resolutions = Arc::new(CjsResolutionStore::default());
  let cache_db = Caches::new(deno_dir_provider.clone());
  let node_analysis_cache = NodeAnalysisCache::new(cache_db.node_analysis_db());
  let cli_node_resolver = Arc::new(CliNodeResolver::new(
    cjs_resolutions.clone(),
    fs.clone(),
    node_resolver.clone(),
    npm_resolver.clone(),
  ));
  let cjs_esm_code_analyzer = CliCjsCodeAnalyzer::new(
    node_analysis_cache,
    fs.clone(),
    cli_node_resolver.clone(),
  );
  let node_code_translator = Arc::new(NodeCodeTranslator::new(
    cjs_esm_code_analyzer,
    deno_runtime::deno_node::DenoFsNodeResolverEnv::new(fs.clone()),
    node_resolver.clone(),
    npm_resolver.clone().into_npm_resolver(),
  ));
  let workspace_resolver = {
    let import_map = match metadata.workspace_resolver.import_map {
      Some(import_map) => Some(
        import_map::parse_from_json_with_options(
          root_dir_url.join(&import_map.specifier).unwrap(),
          &import_map.json,
          import_map::ImportMapOptions {
            address_hook: None,
            expand_imports: true,
          },
        )?
        .import_map,
      ),
      None => None,
    };
    let pkg_jsons = metadata
      .workspace_resolver
      .package_jsons
      .into_iter()
      .map(|(relative_path, json)| {
        let path = root_dir_url
          .join(&relative_path)
          .unwrap()
          .to_file_path()
          .unwrap();
        let pkg_json =
          deno_package_json::PackageJson::load_from_value(path, json);
        Arc::new(pkg_json)
      })
      .collect();
    WorkspaceResolver::new_raw(
      root_dir_url.clone(),
      import_map,
      metadata
        .workspace_resolver
        .jsr_pkgs
        .iter()
        .map(|pkg| ResolverWorkspaceJsrPackage {
          is_patch: false, // only used for enhancing the diagnostic, which isn't shown in deno compile
          base: root_dir_url.join(&pkg.relative_base).unwrap(),
          name: pkg.name.clone(),
          version: pkg.version.clone(),
          exports: pkg.exports.clone(),
        })
        .collect(),
      pkg_jsons,
      metadata.workspace_resolver.pkg_json_resolution,
    )
  };
  let module_loader_factory = StandaloneModuleLoaderFactory {
    shared: Arc::new(SharedModuleLoaderState {
      eszip: WorkspaceEszip {
        eszip,
        root_dir_url,
      },
      workspace_resolver,
      node_resolver: cli_node_resolver.clone(),
      npm_module_loader: Arc::new(NpmModuleLoader::new(
        cjs_resolutions,
        node_code_translator,
        fs.clone(),
        cli_node_resolver,
      )),
    }),
  };

  let permissions = {
    let mut permissions =
      metadata.permissions.to_options(/* cli_arg_urls */ &[]);
    // if running with an npm vfs, grant read access to it
    if let Some(vfs_root) = maybe_vfs_root {
      match &mut permissions.allow_read {
        Some(vec) if vec.is_empty() => {
          // do nothing, already granted
        }
        Some(vec) => {
          vec.push(vfs_root.to_string_lossy().to_string());
        }
        None => {
          permissions.allow_read =
            Some(vec![vfs_root.to_string_lossy().to_string()]);
        }
      }
    }

    let desc_parser =
      Arc::new(RuntimePermissionDescriptorParser::new(fs.clone()));
    let permissions =
      Permissions::from_options(desc_parser.as_ref(), &permissions)?;
    PermissionsContainer::new(desc_parser, permissions)
  };
  let feature_checker = Arc::new({
    let mut checker = FeatureChecker::default();
    checker.set_exit_cb(Box::new(crate::unstable_exit_cb));
    for feature in metadata.unstable_config.features {
      // `metadata` is valid for the whole lifetime of the program, so we
      // can leak the string here.
      checker.enable_feature(feature.leak());
    }
    checker
  });
  let worker_factory = CliMainWorkerFactory::new(
    Arc::new(BlobStore::default()),
    // Code cache is not supported for standalone binary yet.
    None,
    feature_checker,
    fs,
    None,
    None,
    None,
    Box::new(module_loader_factory),
    node_resolver,
    npm_resolver,
    root_cert_store_provider,
    permissions,
    StorageKeyResolver::empty(),
    crate::args::DenoSubcommand::Run(Default::default()),
    CliMainWorkerOptions {
      argv: metadata.argv,
      log_level: WorkerLogLevel::Info,
      enable_op_summary_metrics: false,
      enable_testing_features: false,
      has_node_modules_dir,
      hmr: false,
      inspect_brk: false,
      inspect_wait: false,
      strace_ops: None,
      is_inspecting: false,
      is_npm_main: main_module.scheme() == "npm",
      skip_op_registration: true,
      location: metadata.location,
      argv0: NpmPackageReqReference::from_specifier(&main_module)
        .ok()
        .map(|req_ref| npm_pkg_req_ref_to_binary_command(&req_ref))
        .or(std::env::args().next()),
      node_debug: std::env::var("NODE_DEBUG").ok(),
      origin_data_folder_path: None,
      seed: metadata.seed,
      unsafely_ignore_certificate_errors: metadata
        .unsafely_ignore_certificate_errors,
      create_hmr_runner: None,
      create_coverage_collector: None,
      node_ipc: None,
      serve_port: None,
      serve_host: None,
    },
  );

  // Initialize v8 once from the main thread.
  v8_set_flags(construct_v8_flags(&[], &metadata.v8_flags, vec![]));
  // TODO(bartlomieju): remove last argument once Deploy no longer needs it
  deno_core::JsRuntime::init_platform(None, true);

  let mut worker = worker_factory
    .create_main_worker(WorkerExecutionMode::Run, main_module)
    .await?;

  let exit_code = worker.run().await?;
  Ok(exit_code)
}
