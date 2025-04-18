// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;

use deno_cache_dir::npm::NpmCacheDir;
use deno_config::workspace::ResolverWorkspaceJsrPackage;
use deno_core::error::AnyError;
use deno_core::error::ModuleLoaderError;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_core::v8_set_flags;
use deno_core::FastString;
use deno_core::FeatureChecker;
use deno_core::ModuleLoader;
use deno_core::ModuleSourceCode;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::SourceCodeCacheInfo;
use deno_error::JsErrorBox;
use deno_lib::args::get_root_cert_store;
use deno_lib::args::npm_pkg_req_ref_to_binary_command;
use deno_lib::args::CaData;
use deno_lib::args::RootCertStoreLoadError;
use deno_lib::loader::NpmModuleLoader;
use deno_lib::npm::create_npm_process_state_provider;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::npm::NpmRegistryReadPermissionCheckerMode;
use deno_lib::standalone::binary::NodeModules;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::util::text_encoding::from_utf8_lossy_cow;
use deno_lib::util::text_encoding::from_utf8_lossy_owned;
use deno_lib::util::v8::construct_v8_flags;
use deno_lib::worker::CreateModuleLoaderResult;
use deno_lib::worker::LibMainWorkerFactory;
use deno_lib::worker::LibMainWorkerOptions;
use deno_lib::worker::ModuleLoaderFactory;
use deno_lib::worker::StorageKeyResolver;
use deno_media_type::MediaType;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_package_json::PackageJsonDepValue;
use deno_resolver::cjs::CjsTracker;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use deno_resolver::npm::managed::ManagedNpmResolverCreateOptions;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolver;
use deno_resolver::npm::NpmReqResolverOptions;
use deno_resolver::npm::NpmResolver;
use deno_resolver::npm::NpmResolverCreateOptions;
use deno_resolver::workspace::MappedResolution;
use deno_resolver::workspace::SloppyImportsOptions;
use deno_resolver::workspace::WorkspaceResolver;
use deno_resolver::DenoResolveErrorKind;
use deno_runtime::code_cache::CodeCache;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::WorkerLogLevel;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::analyze::CjsModuleExportAnalyzer;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::cache::NodeResolutionSys;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolver;
use node_resolver::PackageJsonResolver;
use node_resolver::PackageJsonThreadLocalCache;
use node_resolver::ResolutionMode;

use crate::binary::DenoCompileModuleSource;
use crate::binary::StandaloneData;
use crate::binary::StandaloneModules;
use crate::code_cache::DenoCompileCodeCache;
use crate::file_system::DenoRtSys;
use crate::file_system::FileBackedVfs;
use crate::node::CjsCodeAnalyzer;
use crate::node::DenoRtCjsTracker;
use crate::node::DenoRtNodeCodeTranslator;
use crate::node::DenoRtNodeResolver;
use crate::node::DenoRtNpmModuleLoader;
use crate::node::DenoRtNpmReqResolver;

struct SharedModuleLoaderState {
  cjs_tracker: Arc<DenoRtCjsTracker>,
  code_cache: Option<Arc<DenoCompileCodeCache>>,
  modules: Arc<StandaloneModules>,
  node_code_translator: Arc<DenoRtNodeCodeTranslator>,
  node_resolver: Arc<DenoRtNodeResolver>,
  npm_module_loader: Arc<DenoRtNpmModuleLoader>,
  npm_registry_permission_checker: NpmRegistryReadPermissionChecker<DenoRtSys>,
  npm_req_resolver: Arc<DenoRtNpmReqResolver>,
  vfs: Arc<FileBackedVfs>,
  workspace_resolver: WorkspaceResolver<DenoRtSys>,
}

impl SharedModuleLoaderState {
  fn get_code_cache(
    &self,
    specifier: &Url,
    source: &[u8],
  ) -> Option<SourceCodeCacheInfo> {
    let Some(code_cache) = &self.code_cache else {
      return None;
    };
    if !code_cache.enabled() {
      return None;
    }
    // deno version is already included in the root cache key
    let hash = FastInsecureHasher::new_without_deno_version()
      .write_hashable(source)
      .finish();
    let data = code_cache.get_sync(
      specifier,
      deno_runtime::code_cache::CodeCacheType::EsModule,
      hash,
    );
    Some(SourceCodeCacheInfo {
      hash,
      data: data.map(Cow::Owned),
    })
  }
}

#[derive(Clone)]
struct EmbeddedModuleLoader {
  shared: Arc<SharedModuleLoaderState>,
}

impl std::fmt::Debug for EmbeddedModuleLoader {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EmbeddedModuleLoader").finish()
  }
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    raw_specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<Url, ModuleLoaderError> {
    let referrer = if referrer == "." {
      let current_dir = std::env::current_dir().unwrap();
      deno_core::resolve_path(".", &current_dir)
        .map_err(JsErrorBox::from_err)?
    } else {
      Url::parse(referrer).map_err(|err| {
        JsErrorBox::type_error(format!(
          "Referrer uses invalid specifier: {}",
          err
        ))
      })?
    };
    let referrer_kind = if self
      .shared
      .cjs_tracker
      .is_maybe_cjs(&referrer, MediaType::from_specifier(&referrer))
      .map_err(JsErrorBox::from_err)?
    {
      ResolutionMode::Require
    } else {
      ResolutionMode::Import
    };

    if self.shared.node_resolver.in_npm_package(&referrer) {
      return Ok(
        self
          .shared
          .node_resolver
          .resolve(
            raw_specifier,
            &referrer,
            referrer_kind,
            NodeResolutionKind::Execution,
          )
          .and_then(|res| res.into_url())
          .map_err(JsErrorBox::from_err)?,
      );
    }

    let mapped_resolution = self.shared.workspace_resolver.resolve(
      raw_specifier,
      &referrer,
      deno_resolver::workspace::ResolutionKind::Execution,
    );

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
          .resolve_package_subpath_from_deno_module(
            pkg_json.dir_path(),
            sub_path.as_deref(),
            Some(&referrer),
            referrer_kind,
            NodeResolutionKind::Execution,
          )
          .map_err(JsErrorBox::from_err)
          .and_then(|url_or_path| {
            url_or_path.into_url().map_err(JsErrorBox::from_err)
          })?,
      ),
      Ok(MappedResolution::PackageJson {
        dep_result,
        sub_path,
        alias,
        ..
      }) => match dep_result
        .as_ref()
        .map_err(|e| JsErrorBox::from_err(e.clone()))?
      {
        PackageJsonDepValue::File(_) => Err(
          JsErrorBox::from_err(
            DenoResolveErrorKind::UnsupportedPackageJsonFileSpecifier
              .into_box(),
          )
          .into(),
        ),
        PackageJsonDepValue::Req(req) => Ok(
          self
            .shared
            .npm_req_resolver
            .resolve_req_with_sub_path(
              req,
              sub_path.as_deref(),
              &referrer,
              referrer_kind,
              NodeResolutionKind::Execution,
            )
            .map_err(JsErrorBox::from_err)
            .and_then(|url_or_path| {
              url_or_path.into_url().map_err(JsErrorBox::from_err)
            })?,
        ),
        PackageJsonDepValue::Workspace(version_req) => {
          let pkg_folder = self
            .shared
            .workspace_resolver
            .resolve_workspace_pkg_json_folder_for_pkg_json_dep(
              alias,
              version_req,
            )
            .map_err(JsErrorBox::from_err)?;
          Ok(
            self
              .shared
              .node_resolver
              .resolve_package_subpath_from_deno_module(
                pkg_folder,
                sub_path.as_deref(),
                Some(&referrer),
                referrer_kind,
                NodeResolutionKind::Execution,
              )
              .map_err(JsErrorBox::from_err)
              .and_then(|url_or_path| {
                url_or_path.into_url().map_err(JsErrorBox::from_err)
              })?,
          )
        }
      },
      Ok(MappedResolution::Normal { specifier, .. }) => {
        if let Ok(reference) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          return Ok(
            self
              .shared
              .npm_req_resolver
              .resolve_req_reference(
                &reference,
                &referrer,
                referrer_kind,
                NodeResolutionKind::Execution,
              )
              .map_err(JsErrorBox::from_err)
              .and_then(|url_or_path| {
                url_or_path.into_url().map_err(JsErrorBox::from_err)
              })?,
          );
        }

        if specifier.scheme() == "jsr" {
          if let Some(specifier) = self
            .shared
            .modules
            .resolve_specifier(&specifier)
            .map_err(JsErrorBox::from_err)?
          {
            return Ok(specifier.clone());
          }
        }

        Ok(
          self
            .shared
            .node_resolver
            .handle_if_in_node_modules(&specifier)
            .unwrap_or(specifier),
        )
      }
      Err(err)
        if err.is_unmapped_bare_specifier() && referrer.scheme() == "file" =>
      {
        let maybe_res = self
          .shared
          .npm_req_resolver
          .resolve_if_for_npm_pkg(
            raw_specifier,
            &referrer,
            referrer_kind,
            NodeResolutionKind::Execution,
          )
          .map_err(JsErrorBox::from_err)?;
        if let Some(res) = maybe_res {
          return Ok(res.into_url().map_err(JsErrorBox::from_err)?);
        }
        Err(JsErrorBox::from_err(err).into())
      }
      Err(err) => Err(JsErrorBox::from_err(err).into()),
    }
  }

  fn get_host_defined_options<'s>(
    &self,
    scope: &mut deno_core::v8::HandleScope<'s>,
    name: &str,
  ) -> Option<deno_core::v8::Local<'s, deno_core::v8::Data>> {
    let name = Url::parse(name).ok()?;
    if self.shared.node_resolver.in_npm_package(&name) {
      Some(create_host_defined_options(scope))
    } else {
      None
    }
  }

  fn load(
    &self,
    original_specifier: &Url,
    maybe_referrer: Option<&Url>,
    _is_dynamic: bool,
    _requested_module_type: RequestedModuleType,
  ) -> deno_core::ModuleLoadResponse {
    if original_specifier.scheme() == "data" {
      let data_url_text =
        match deno_media_type::data_url::RawDataUrl::parse(original_specifier)
          .and_then(|url| url.decode())
        {
          Ok(response) => response,
          Err(err) => {
            return deno_core::ModuleLoadResponse::Sync(Err(
              JsErrorBox::type_error(format!("{:#}", err)).into(),
            ));
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
      let shared = self.shared.clone();
      let original_specifier = original_specifier.clone();
      let maybe_referrer = maybe_referrer.cloned();
      return deno_core::ModuleLoadResponse::Async(
        async move {
          let code_source = shared
            .npm_module_loader
            .load(&original_specifier, maybe_referrer.as_ref())
            .await
            .map_err(JsErrorBox::from_err)?;
          let code_cache_entry = shared.get_code_cache(
            &code_source.found_url,
            code_source.code.as_bytes(),
          );
          Ok(deno_core::ModuleSource::new_with_redirect(
            match code_source.media_type {
              MediaType::Json => ModuleType::Json,
              _ => ModuleType::JavaScript,
            },
            code_source.code,
            &original_specifier,
            &code_source.found_url,
            code_cache_entry,
          ))
        }
        .boxed_local(),
      );
    }

    match self.shared.modules.read(original_specifier) {
      Ok(Some(module)) => {
        let media_type = module.media_type;
        let (module_specifier, module_type, module_source) =
          module.into_parts();
        let is_maybe_cjs = match self
          .shared
          .cjs_tracker
          .is_maybe_cjs(original_specifier, media_type)
        {
          Ok(is_maybe_cjs) => is_maybe_cjs,
          Err(err) => {
            return deno_core::ModuleLoadResponse::Sync(Err(
              JsErrorBox::type_error(format!("{:?}", err)).into(),
            ));
          }
        };
        if is_maybe_cjs {
          let original_specifier = original_specifier.clone();
          let module_specifier = module_specifier.clone();
          let shared = self.shared.clone();
          deno_core::ModuleLoadResponse::Async(
            async move {
              let source = match module_source {
                DenoCompileModuleSource::String(string) => {
                  Cow::Borrowed(string)
                }
                DenoCompileModuleSource::Bytes(module_code_bytes) => {
                  match module_code_bytes {
                    Cow::Owned(bytes) => {
                      Cow::Owned(from_utf8_lossy_owned(bytes))
                    }
                    Cow::Borrowed(bytes) => String::from_utf8_lossy(bytes),
                  }
                }
              };
              let source = shared
                .node_code_translator
                .translate_cjs_to_esm(&module_specifier, Some(source))
                .await
                .map_err(JsErrorBox::from_err)?;
              let module_source = match source {
                Cow::Owned(source) => ModuleSourceCode::String(source.into()),
                Cow::Borrowed(source) => {
                  ModuleSourceCode::String(FastString::from_static(source))
                }
              };
              let code_cache_entry = shared
                .get_code_cache(&module_specifier, module_source.as_bytes());
              Ok(deno_core::ModuleSource::new_with_redirect(
                module_type,
                module_source,
                &original_specifier,
                &module_specifier,
                code_cache_entry,
              ))
            }
            .boxed_local(),
          )
        } else {
          let module_source = module_source.into_for_v8();
          let code_cache_entry = self
            .shared
            .get_code_cache(module_specifier, module_source.as_bytes());
          deno_core::ModuleLoadResponse::Sync(Ok(
            deno_core::ModuleSource::new_with_redirect(
              module_type,
              module_source,
              original_specifier,
              module_specifier,
              code_cache_entry,
            ),
          ))
        }
      }
      Ok(None) => deno_core::ModuleLoadResponse::Sync(Err(
        JsErrorBox::type_error(format!(
          "Module not found: {}",
          original_specifier
        ))
        .into(),
      )),
      Err(err) => deno_core::ModuleLoadResponse::Sync(Err(
        JsErrorBox::type_error(format!("{:?}", err)).into(),
      )),
    }
  }

  fn code_cache_ready(
    &self,
    specifier: Url,
    source_hash: u64,
    code_cache_data: &[u8],
  ) -> LocalBoxFuture<'static, ()> {
    if let Some(code_cache) = &self.shared.code_cache {
      code_cache.set_sync(
        specifier,
        deno_runtime::code_cache::CodeCacheType::EsModule,
        source_hash,
        code_cache_data,
      );
    }
    std::future::ready(()).boxed_local()
  }

  fn get_source_map(&self, file_name: &str) -> Option<Cow<[u8]>> {
    let url = Url::parse(file_name).ok()?;
    let data = self.shared.modules.read(&url).ok()??;
    data.source_map
  }

  fn get_source_mapped_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    let specifier = Url::parse(file_name).ok()?;
    let data = self.shared.modules.read(&specifier).ok()??;

    let source = String::from_utf8_lossy(&data.data);
    // Do NOT use .lines(): it skips the terminating empty line.
    // (due to internally using_terminator() instead of .split())
    let lines: Vec<&str> = source.split('\n').collect();
    if line_number >= lines.len() {
      Some(format!(
        "{} Couldn't format source line: Line {} is out of bounds (source may have changed at runtime)",
        crate::colors::yellow("Warning"), line_number + 1,
      ))
    } else {
      Some(lines[line_number].to_string())
    }
  }
}

impl NodeRequireLoader for EmbeddedModuleLoader {
  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn deno_runtime::deno_node::NodePermissions,
    path: &'a std::path::Path,
  ) -> Result<Cow<'a, std::path::Path>, JsErrorBox> {
    if self.shared.modules.has_file(path) {
      // allow reading if the file is in the snapshot
      return Ok(Cow::Borrowed(path));
    }

    self
      .shared
      .npm_registry_permission_checker
      .ensure_read_permission(permissions, path)
      .map_err(JsErrorBox::from_err)
  }

  fn load_text_file_lossy(
    &self,
    path: &std::path::Path,
  ) -> Result<Cow<'static, str>, JsErrorBox> {
    let file_entry = self
      .shared
      .vfs
      .file_entry(path)
      .map_err(JsErrorBox::from_err)?;
    let file_bytes = self
      .shared
      .vfs
      .read_file_offset_with_len(
        file_entry.transpiled_offset.unwrap_or(file_entry.offset),
      )
      .map_err(JsErrorBox::from_err)?;
    Ok(from_utf8_lossy_cow(file_bytes))
  }

  fn is_maybe_cjs(&self, specifier: &Url) -> Result<bool, ClosestPkgJsonError> {
    let media_type = MediaType::from_specifier(specifier);
    self.shared.cjs_tracker.is_maybe_cjs(specifier, media_type)
  }
}

struct StandaloneModuleLoaderFactory {
  shared: Arc<SharedModuleLoaderState>,
}

impl StandaloneModuleLoaderFactory {
  pub fn create_result(&self) -> CreateModuleLoaderResult {
    let loader = Rc::new(EmbeddedModuleLoader {
      shared: self.shared.clone(),
    });
    CreateModuleLoaderResult {
      module_loader: loader.clone(),
      node_require_loader: loader,
    }
  }
}

impl ModuleLoaderFactory for StandaloneModuleLoaderFactory {
  fn create_for_main(
    &self,
    _root_permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    self.create_result()
  }

  fn create_for_worker(
    &self,
    _parent_permissions: PermissionsContainer,
    _permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    self.create_result()
  }
}

struct StandaloneRootCertStoreProvider {
  ca_stores: Option<Vec<String>>,
  ca_data: Option<CaData>,
  cell: OnceLock<Result<RootCertStore, RootCertStoreLoadError>>,
}

impl RootCertStoreProvider for StandaloneRootCertStoreProvider {
  fn get_or_try_init(&self) -> Result<&RootCertStore, JsErrorBox> {
    self
      .cell
      // get_or_try_init was not stable yet when this was written
      .get_or_init(|| {
        get_root_cert_store(None, self.ca_stores.clone(), self.ca_data.clone())
      })
      .as_ref()
      .map_err(|err| JsErrorBox::from_err(err.clone()))
  }
}

pub async fn run(
  fs: Arc<dyn FileSystem>,
  sys: DenoRtSys,
  data: StandaloneData,
) -> Result<i32, AnyError> {
  let StandaloneData {
    metadata,
    modules,
    npm_snapshot,
    root_path,
    vfs,
  } = data;

  let root_cert_store_provider = Arc::new(StandaloneRootCertStoreProvider {
    ca_stores: metadata.ca_stores,
    ca_data: metadata.ca_data.map(CaData::Bytes),
    cell: Default::default(),
  });
  // use a dummy npm registry url
  let npm_registry_url = Url::parse("https://localhost/").unwrap();
  let root_dir_url = Arc::new(Url::from_directory_path(&root_path).unwrap());
  let main_module = root_dir_url.join(&metadata.entrypoint_key).unwrap();
  let npm_global_cache_dir = root_path.join(".deno_compile_node_modules");
  let pkg_json_resolver = Arc::new(PackageJsonResolver::new(
    sys.clone(),
    Some(Arc::new(PackageJsonThreadLocalCache)),
  ));
  let npm_registry_permission_checker = {
    let mode = match &metadata.node_modules {
      Some(NodeModules::Managed {
        node_modules_dir: Some(path),
      }) => NpmRegistryReadPermissionCheckerMode::Local(PathBuf::from(path)),
      Some(NodeModules::Byonm { .. }) => {
        NpmRegistryReadPermissionCheckerMode::Byonm
      }
      Some(NodeModules::Managed {
        node_modules_dir: None,
      })
      | None => NpmRegistryReadPermissionCheckerMode::Global(
        npm_global_cache_dir.clone(),
      ),
    };
    NpmRegistryReadPermissionChecker::new(sys.clone(), mode)
  };
  let node_resolution_sys = NodeResolutionSys::new(sys.clone(), None);
  let (in_npm_pkg_checker, npm_resolver) = match metadata.node_modules {
    Some(NodeModules::Managed { node_modules_dir }) => {
      // create an npmrc that uses the fake npm_registry_url to resolve packages
      let npmrc = Arc::new(ResolvedNpmRc {
        default_config: deno_npm::npm_rc::RegistryConfigWithUrl {
          registry_url: npm_registry_url.clone(),
          config: Default::default(),
        },
        scopes: Default::default(),
        registry_configs: Default::default(),
      });
      let npm_cache_dir = Arc::new(NpmCacheDir::new(
        &sys,
        npm_global_cache_dir,
        npmrc.get_all_known_registries_urls(),
      ));
      let snapshot = npm_snapshot.unwrap();
      let maybe_node_modules_path = node_modules_dir
        .map(|node_modules_dir| root_path.join(node_modules_dir));
      let in_npm_pkg_checker =
        DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Managed(
          ManagedInNpmPkgCheckerCreateOptions {
            root_cache_dir_url: npm_cache_dir.root_dir_url(),
            maybe_node_modules_path: maybe_node_modules_path.as_deref(),
          },
        ));
      let npm_resolution =
        Arc::new(NpmResolutionCell::new(NpmResolutionSnapshot::new(snapshot)));
      let npm_resolver = NpmResolver::<DenoRtSys>::new::<DenoRtSys>(
        NpmResolverCreateOptions::Managed(ManagedNpmResolverCreateOptions {
          npm_resolution,
          npm_cache_dir,
          sys: sys.clone(),
          maybe_node_modules_path,
          npm_system_info: Default::default(),
          npmrc,
        }),
      );
      (in_npm_pkg_checker, npm_resolver)
    }
    Some(NodeModules::Byonm {
      root_node_modules_dir,
    }) => {
      let root_node_modules_dir =
        root_node_modules_dir.map(|p| vfs.root().join(p));
      let in_npm_pkg_checker =
        DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);
      let npm_resolver = NpmResolver::<DenoRtSys>::new::<DenoRtSys>(
        NpmResolverCreateOptions::Byonm(ByonmNpmResolverCreateOptions {
          sys: node_resolution_sys.clone(),
          pkg_json_resolver: pkg_json_resolver.clone(),
          root_node_modules_dir,
        }),
      );
      (in_npm_pkg_checker, npm_resolver)
    }
    None => {
      // Packages from different registries are already inlined in the binary,
      // so no need to create actual `.npmrc` configuration.
      let npmrc = create_default_npmrc();
      let npm_cache_dir = Arc::new(NpmCacheDir::new(
        &sys,
        npm_global_cache_dir,
        npmrc.get_all_known_registries_urls(),
      ));
      let in_npm_pkg_checker =
        DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Managed(
          ManagedInNpmPkgCheckerCreateOptions {
            root_cache_dir_url: npm_cache_dir.root_dir_url(),
            maybe_node_modules_path: None,
          },
        ));
      let npm_resolution = Arc::new(NpmResolutionCell::default());
      let npm_resolver = NpmResolver::<DenoRtSys>::new::<DenoRtSys>(
        NpmResolverCreateOptions::Managed(ManagedNpmResolverCreateOptions {
          npm_resolution,
          sys: sys.clone(),
          npm_cache_dir,
          maybe_node_modules_path: None,
          npm_system_info: Default::default(),
          npmrc: create_default_npmrc(),
        }),
      );
      (in_npm_pkg_checker, npm_resolver)
    }
  };

  let has_node_modules_dir = npm_resolver.root_node_modules_path().is_some();
  let node_resolver = Arc::new(NodeResolver::new(
    in_npm_pkg_checker.clone(),
    DenoIsBuiltInNodeModuleChecker,
    npm_resolver.clone(),
    pkg_json_resolver.clone(),
    node_resolution_sys,
    node_resolver::NodeResolverOptions::default(),
  ));
  let cjs_tracker = Arc::new(CjsTracker::new(
    in_npm_pkg_checker.clone(),
    pkg_json_resolver.clone(),
    if metadata.unstable_config.detect_cjs {
      IsCjsResolutionMode::ImplicitTypeCommonJs
    } else if metadata.workspace_resolver.package_jsons.is_empty() {
      IsCjsResolutionMode::Disabled
    } else {
      IsCjsResolutionMode::ExplicitTypeCommonJs
    },
  ));
  let npm_req_resolver = Arc::new(NpmReqResolver::new(NpmReqResolverOptions {
    sys: sys.clone(),
    in_npm_pkg_checker: in_npm_pkg_checker.clone(),
    node_resolver: node_resolver.clone(),
    npm_resolver: npm_resolver.clone(),
  }));
  let cjs_esm_code_analyzer =
    CjsCodeAnalyzer::new(cjs_tracker.clone(), modules.clone(), sys.clone());
  let cjs_module_export_analyzer = Arc::new(CjsModuleExportAnalyzer::new(
    cjs_esm_code_analyzer,
    in_npm_pkg_checker,
    node_resolver.clone(),
    npm_resolver.clone(),
    pkg_json_resolver.clone(),
    sys.clone(),
  ));
  let node_code_translator =
    Arc::new(NodeCodeTranslator::new(cjs_module_export_analyzer));
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
          deno_package_json::PackageJson::load_from_value(path, json)?;
        Ok(Arc::new(pkg_json))
      })
      .collect::<Result<Vec<_>, AnyError>>()?;
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
      if metadata.unstable_config.sloppy_imports {
        SloppyImportsOptions::Enabled
      } else {
        SloppyImportsOptions::Disabled
      },
      Default::default(),
      Default::default(),
      Default::default(),
      sys.clone(),
    )
  };
  let code_cache = match metadata.code_cache_key {
    Some(code_cache_key) => Some(Arc::new(DenoCompileCodeCache::new(
      root_path.with_file_name(format!(
        "{}.cache",
        root_path.file_name().unwrap().to_string_lossy()
      )),
      code_cache_key,
    ))),
    None => {
      log::debug!("Code cache disabled.");
      None
    }
  };
  let module_loader_factory = StandaloneModuleLoaderFactory {
    shared: Arc::new(SharedModuleLoaderState {
      cjs_tracker: cjs_tracker.clone(),
      code_cache: code_cache.clone(),
      modules,
      node_code_translator: node_code_translator.clone(),
      node_resolver: node_resolver.clone(),
      npm_module_loader: Arc::new(NpmModuleLoader::new(
        cjs_tracker.clone(),
        node_code_translator,
        sys.clone(),
      )),
      npm_registry_permission_checker,
      npm_req_resolver,
      vfs: vfs.clone(),
      workspace_resolver,
    }),
  };

  let permissions = {
    let mut permissions = metadata.permissions;
    // grant read access to the vfs
    match &mut permissions.allow_read {
      Some(vec) if vec.is_empty() => {
        // do nothing, already granted
      }
      Some(vec) => {
        vec.push(root_path.to_string_lossy().to_string());
      }
      None => {
        permissions.allow_read =
          Some(vec![root_path.to_string_lossy().to_string()]);
      }
    }

    let desc_parser =
      Arc::new(RuntimePermissionDescriptorParser::new(sys.clone()));
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
  let lib_main_worker_options = LibMainWorkerOptions {
    argv: metadata.argv,
    log_level: WorkerLogLevel::Info,
    enable_op_summary_metrics: false,
    enable_testing_features: false,
    has_node_modules_dir,
    inspect_brk: false,
    inspect_wait: false,
    strace_ops: None,
    is_inspecting: false,
    is_standalone: true,
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
    node_ipc: None,
    serve_port: None,
    serve_host: None,
    otel_config: metadata.otel_config,
    no_legacy_abort: false,
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
  };
  let worker_factory = LibMainWorkerFactory::new(
    Arc::new(BlobStore::default()),
    code_cache.map(|c| c.for_deno_core()),
    Some(sys.as_deno_rt_native_addon_loader()),
    feature_checker,
    fs,
    None,
    Box::new(module_loader_factory),
    node_resolver.clone(),
    create_npm_process_state_provider(&npm_resolver),
    pkg_json_resolver,
    root_cert_store_provider,
    StorageKeyResolver::empty(),
    sys.clone(),
    lib_main_worker_options,
  );

  // Initialize v8 once from the main thread.
  v8_set_flags(construct_v8_flags(&[], &metadata.v8_flags, vec![]));
  // TODO(bartlomieju): remove last argument once Deploy no longer needs it
  deno_core::JsRuntime::init_platform(None, true);

  let main_module = match NpmPackageReqReference::from_specifier(&main_module) {
    Ok(package_ref) => {
      let pkg_folder = npm_resolver.resolve_pkg_folder_from_deno_module_req(
        package_ref.req(),
        &deno_path_util::url_from_file_path(&vfs.root().join("package.json"))?,
      )?;
      worker_factory
        .resolve_npm_binary_entrypoint(&pkg_folder, package_ref.sub_path())?
    }
    Err(_) => main_module,
  };

  let mut worker = worker_factory.create_main_worker(
    WorkerExecutionMode::Run,
    permissions,
    main_module,
  )?;

  let exit_code = worker.run().await?;
  Ok(exit_code)
}

fn create_default_npmrc() -> Arc<ResolvedNpmRc> {
  // this is fine because multiple registries are combined into
  // one when compiling the binary
  Arc::new(ResolvedNpmRc {
    default_config: deno_npm::npm_rc::RegistryConfigWithUrl {
      registry_url: Url::parse("https://registry.npmjs.org").unwrap(),
      config: Default::default(),
    },
    scopes: Default::default(),
    registry_configs: Default::default(),
  })
}
