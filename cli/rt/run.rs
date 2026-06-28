// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;

use deno_cache_dir::npm::NpmCacheDir;
use deno_config::workspace::ResolverWorkspaceJsrPackage;
use deno_core::FastString;
use deno_core::ModuleLoadOptions;
use deno_core::ModuleLoadReferrer;
use deno_core::ModuleLoader;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::SourceCodeCacheInfo;
use deno_core::error::AnyError;
use deno_core::error::ModuleLoaderError;
use deno_core::futures::FutureExt;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::url::Url;
use deno_core::v8_set_flags;
use deno_error::JsErrorBox;
use deno_lib::args::CaData;
use deno_lib::args::RootCertStoreLoadError;
use deno_lib::args::get_root_cert_store;
use deno_lib::args::npm_pkg_req_ref_to_binary_command;
use deno_lib::loader::as_deno_resolver_requested_module_type;
use deno_lib::loader::loaded_module_source_to_module_source_code;
use deno_lib::loader::module_type_from_media_and_requested_type;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::npm::NpmRegistryReadPermissionCheckerMode;
use deno_lib::npm::create_npm_process_state_provider;
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
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npmrc::ResolvedNpmRc;
use deno_package_json::PackageJsonDepValue;
use deno_resolver::DenoResolveErrorKind;
use deno_resolver::cjs::CjsTracker;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::loader::NpmModuleLoader;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolver;
use deno_resolver::npm::NpmReqResolverOptions;
use deno_resolver::npm::NpmResolver;
use deno_resolver::npm::NpmResolverCreateOptions;
use deno_resolver::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use deno_resolver::npm::managed::ManagedNpmResolverCreateOptions;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::workspace::MappedResolution;
use deno_resolver::workspace::SloppyImportsOptions;
use deno_resolver::workspace::WorkspaceResolver;
use deno_runtime::FeatureChecker;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::WorkerLogLevel;
use deno_runtime::code_cache::CodeCache;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::ops::module_hooks::LoaderHookRegistry;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::Blob;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolver;
use node_resolver::PackageJsonResolver;
use node_resolver::PackageJsonThreadLocalCache;
use node_resolver::ResolutionMode;
use node_resolver::analyze::CjsModuleExportAnalyzer;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::cache::NodeResolutionSys;
use node_resolver::errors::PackageJsonLoadError;
use sys_traits::EnvCurrentDir;

use crate::binary::DenoCompileModuleSource;
use crate::binary::StandaloneData;
use crate::binary::StandaloneModules;
use crate::binary::transpile_runtime_module;
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
  blob_store: Arc<BlobStore>,
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
  hook_registry: LoaderHookRegistry,
  sys: DenoRtSys,
  /// For blob/object-URL module workers, the captured root blob and its
  /// specifier. Used so the worker's root module load resolves from the
  /// synchronously-captured blob even if `URL.revokeObjectURL` has since
  /// removed it from the blob store.
  maybe_main_module_blob: Option<(ModuleSpecifier, Arc<Blob>)>,
}

impl std::fmt::Debug for EmbeddedModuleLoader {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EmbeddedModuleLoader").finish()
  }
}

impl EmbeddedModuleLoader {
  fn resolve_inner(
    &self,
    raw_specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<Url, ModuleLoaderError> {
    let referrer = if referrer == "." {
      #[allow(
        clippy::disallowed_methods,
        reason = "ok to use current_dir here"
      )]
      let current_dir =
        self.sys.env_current_dir().map_err(JsErrorBox::from_err)?;
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
    let resolution_mode = if self
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
      return self
        .shared
        .node_resolver
        .resolve(
          raw_specifier,
          &referrer,
          resolution_mode,
          NodeResolutionKind::Execution,
        )
        .and_then(|res| res.into_url())
        .map_err(JsErrorBox::from_err);
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
            resolution_mode,
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
        PackageJsonDepValue::File(_) => Err(JsErrorBox::from_err(
          DenoResolveErrorKind::UnsupportedPackageJsonFileSpecifier.into_box(),
        )),
        PackageJsonDepValue::Req(req) => Ok(
          self
            .shared
            .npm_req_resolver
            .resolve_req_with_sub_path(
              req,
              sub_path.as_deref(),
              &referrer,
              resolution_mode,
              NodeResolutionKind::Execution,
            )
            .map_err(JsErrorBox::from_err)
            .and_then(|url_or_path| {
              url_or_path.into_url().map_err(JsErrorBox::from_err)
            })?,
        ),
        PackageJsonDepValue::Workspace { name, version_req } => {
          let pkg_folder = self
            .shared
            .workspace_resolver
            .resolve_workspace_pkg_json_folder_for_pkg_json_dep(
              name.as_deref().unwrap_or(alias),
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
                resolution_mode,
                NodeResolutionKind::Execution,
              )
              .map_err(JsErrorBox::from_err)
              .and_then(|url_or_path| {
                url_or_path.into_url().map_err(JsErrorBox::from_err)
              })?,
          )
        }
        PackageJsonDepValue::Catalog(catalog_name) => {
          match self
            .shared
            .workspace_resolver
            .resolve_catalog_dep(alias, catalog_name)
          {
            Some(req) => Ok(
              self
                .shared
                .npm_req_resolver
                .resolve_req_with_sub_path(
                  &req,
                  sub_path.as_deref(),
                  &referrer,
                  resolution_mode,
                  NodeResolutionKind::Execution,
                )
                .map_err(JsErrorBox::from_err)
                .and_then(|url_or_path| {
                  url_or_path.into_url().map_err(JsErrorBox::from_err)
                })?,
            ),
            None => Err(JsErrorBox::generic(format!(
              "Package '{}' not found in catalog",
              alias
            ))),
          }
        }
      },
      Ok(MappedResolution::PackageJsonImport { pkg_json }) => self
        .shared
        .node_resolver
        .resolve_package_import(
          raw_specifier,
          Some(&node_resolver::UrlOrPathRef::from_url(&referrer)),
          Some(pkg_json),
          resolution_mode,
          NodeResolutionKind::Execution,
        )
        .map_err(JsErrorBox::from_err)
        .and_then(|url_or_path| {
          url_or_path.into_url().map_err(JsErrorBox::from_err)
        }),
      Ok(MappedResolution::Normal { specifier, .. }) => {
        if let Ok(reference) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          return self
            .shared
            .npm_req_resolver
            .resolve_req_reference(
              &reference,
              &referrer,
              resolution_mode,
              NodeResolutionKind::Execution,
            )
            .map_err(JsErrorBox::from_err)
            .and_then(|url_or_path| {
              url_or_path.into_url().map_err(JsErrorBox::from_err)
            });
        }

        if specifier.scheme() == "jsr"
          && let Some(specifier) = self
            .shared
            .modules
            .resolve_specifier(&specifier)
            .map_err(JsErrorBox::from_err)?
        {
          return Ok(specifier.clone());
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
            resolution_mode,
            NodeResolutionKind::Execution,
          )
          .map_err(JsErrorBox::from_err)?;
        if let Some(res) = maybe_res {
          return res.into_url().map_err(JsErrorBox::from_err);
        }
        Err(JsErrorBox::from_err(err))
      }
      Err(err) => Err(JsErrorBox::from_err(err)),
    }
  }

  /// Load a module using the default embedded/filesystem logic, including
  /// the async npm-package path. Used both by `ModuleLoader::load` after
  /// data-URL/hook routing and by the hook-fallthrough branch when a load
  /// hook calls `nextLoad()` without short-circuiting.
  fn load_default(
    &self,
    original_specifier: &Url,
    maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> deno_core::ModuleLoadResponse {
    if self.shared.node_resolver.in_npm_package(original_specifier) {
      let shared = self.shared.clone();
      let original_specifier = original_specifier.clone();
      let maybe_referrer = maybe_referrer.map(|r| r.specifier.clone());
      return deno_core::ModuleLoadResponse::Async(
        async move {
          let code_source = shared
            .npm_module_loader
            .load(
              Cow::Borrowed(&original_specifier),
              maybe_referrer.as_ref(),
              &as_deno_resolver_requested_module_type(
                &options.requested_module_type,
              ),
            )
            .await
            .map_err(JsErrorBox::from_err)?;
          let module_type = module_type_from_media_and_requested_type(
            code_source.media_type,
            &options.requested_module_type,
          );
          // Only JavaScript modules produce a V8 code cache. Requesting one
          // for other module types (JSON, Wasm, etc.) bumps the `FirstRun`
          // strategy's pending counter via `get_sync` without a matching
          // `set_sync`, preventing the cache from ever being serialized.
          // See https://github.com/denoland/deno/issues/31766
          let code_cache_entry = if module_type == ModuleType::JavaScript {
            shared.get_code_cache(
              &code_source.specifier,
              code_source.source.as_bytes(),
            )
          } else {
            None
          };
          Ok(deno_core::ModuleSource::new_with_redirect(
            module_type,
            loaded_module_source_to_module_source_code(code_source.source),
            &original_specifier,
            &code_source.specifier,
            code_cache_entry,
          ))
        }
        .boxed_local(),
      );
    }

    match self.shared.modules.read(original_specifier) {
      Ok(Some(module)) => {
        match options.requested_module_type {
          RequestedModuleType::Text | RequestedModuleType::Bytes => {
            let module_source = DenoCompileModuleSource::Bytes(module.data);
            return deno_core::ModuleLoadResponse::Sync(Ok(
              deno_core::ModuleSource::new_with_redirect(
                match options.requested_module_type {
                  RequestedModuleType::Text => ModuleType::Text,
                  RequestedModuleType::Bytes => ModuleType::Bytes,
                  _ => unreachable!(),
                },
                match options.requested_module_type {
                  RequestedModuleType::Text => module_source.into_for_v8(),
                  RequestedModuleType::Bytes => {
                    ModuleSourceCode::Bytes(module_source.into_bytes_for_v8())
                  }
                  _ => unreachable!(),
                },
                original_specifier,
                module.specifier,
                None,
              ),
            ));
          }
          RequestedModuleType::Other(_)
          | RequestedModuleType::None
          | RequestedModuleType::Json => {
            // ignore
          }
        }

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
              JsErrorBox::type_error(format!("{:?}", err)),
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
              // CJS modules are always JavaScript, but gate on the module
              // type anyway to keep the code cache contract uniform across all
              // load paths: only JavaScript produces a V8 code cache.
              // See https://github.com/denoland/deno/issues/31766
              let code_cache_entry = if module_type == ModuleType::JavaScript {
                shared
                  .get_code_cache(&module_specifier, module_source.as_bytes())
              } else {
                None
              };
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
          // Only JavaScript modules produce a V8 code cache. Requesting the
          // code cache for other module types (JSON, Wasm, etc.) would still
          // bump the `FirstRun` strategy's pending counter via `get_sync`
          // without a matching `set_sync`, preventing the cache from ever
          // being serialized. See https://github.com/denoland/deno/issues/31766
          let code_cache_entry = if module_type == ModuleType::JavaScript {
            self
              .shared
              .get_code_cache(module_specifier, module_source.as_bytes())
          } else {
            None
          };
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
      Ok(None) => {
        // Fall back to reading from disk (used by dev entrypoint in HMR mode).
        if original_specifier.scheme() == "file"
          && let Ok(path) = original_specifier.to_file_path()
          && let Ok(source) = std::fs::read_to_string(&path)
        {
          let media_type = MediaType::from_specifier(original_specifier);
          let (module_type, should_transpile) = match media_type {
            MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
              (ModuleType::JavaScript, false)
            }
            MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Jsx
            | MediaType::Tsx => (ModuleType::JavaScript, true),
            MediaType::Json => (ModuleType::Json, false),
            _ => (ModuleType::JavaScript, false),
          };
          let code = if should_transpile {
            match deno_ast::parse_module(deno_ast::ParseParams {
              specifier: original_specifier.clone(),
              text: source.into(),
              media_type,
              capture_tokens: false,
              scope_analysis: false,
              maybe_syntax: None,
            }) {
              Ok(parsed) => match parsed.transpile(
                &deno_ast::TranspileOptions::default(),
                &deno_ast::TranspileModuleOptions::default(),
                &deno_ast::EmitOptions::default(),
              ) {
                Ok(transpiled) => {
                  ModuleSourceCode::String(transpiled.into_source().text.into())
                }
                Err(e) => {
                  return deno_core::ModuleLoadResponse::Sync(Err(
                    JsErrorBox::type_error(format!("Transpile error: {e}")),
                  ));
                }
              },
              Err(e) => {
                return deno_core::ModuleLoadResponse::Sync(Err(
                  JsErrorBox::type_error(format!("Parse error: {e}")),
                ));
              }
            }
          } else {
            ModuleSourceCode::String(source.into())
          };
          return deno_core::ModuleLoadResponse::Sync(Ok(
            deno_core::ModuleSource::new(
              module_type,
              code,
              original_specifier,
              None,
            ),
          ));
        }
        deno_core::ModuleLoadResponse::Sync(Err(JsErrorBox::type_error(
          format!("Module not found: {}", original_specifier),
        )))
      }
      Err(err) => deno_core::ModuleLoadResponse::Sync(Err(
        JsErrorBox::type_error(format!("{:?}", err)),
      )),
    }
  }
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    raw_specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    self.resolve_inner(raw_specifier, referrer, kind)
  }

  fn resolve_with_scope(
    &self,
    scope: &mut deno_core::v8::PinScope,
    raw_specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
    import_attributes: &std::collections::HashMap<String, String>,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    // CJS modules generated by Deno import `node:module` internally. Node does
    // not expose that implementation detail to resolve hooks, so only bypass
    // hooks for CJS referrers. User ESM imports of `node:module` still go
    // through hooks.
    if raw_specifier == "node:module"
      && let Ok(referrer) = Url::parse(referrer)
      && self.shared.cjs_tracker.get_referrer_kind(&referrer)
        == ResolutionMode::Require
    {
      return self.resolve_inner(raw_specifier, referrer.as_str(), kind);
    }
    if raw_specifier == "node:module"
      && let Ok(referrer) = Url::parse(referrer)
      && self.is_maybe_cjs(&referrer).unwrap_or(false)
    {
      return self.resolve_inner(raw_specifier, referrer.as_str(), kind);
    }
    let resolved = if let Some(url) = self.hook_registry.resolve(
      scope,
      raw_specifier,
      referrer,
      import_attributes,
    )? {
      ModuleSpecifier::parse(&url).map_err(JsErrorBox::from_err)
    } else {
      self.resolve_inner(raw_specifier, referrer, kind)
    };
    // Stash the full import attributes against the resolved URL so the load
    // hook (which V8 only hands the `type` attribute) can recover them.
    if let Ok(resolved) = &resolved {
      self
        .hook_registry
        .record_resolved_attributes(resolved.as_str(), import_attributes);
    }
    resolved
  }

  fn pump_event_loop_during_load(&self) -> bool {
    // Load hooks respond through the async bridge, so the event loop must be
    // pumped during the recursive load or the load deadlocks.
    self.hook_registry.load_active.get()
  }

  fn get_host_defined_options<'s>(
    &self,
    scope: &mut deno_core::v8::PinScope<'s, '_>,
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
    maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> deno_core::ModuleLoadResponse {
    if original_specifier.scheme() == "data" {
      let raw_data_url = match deno_media_type::data_url::RawDataUrl::parse(
        original_specifier,
      ) {
        Ok(raw) => raw,
        Err(err) => {
          return deno_core::ModuleLoadResponse::Sync(Err(
            JsErrorBox::type_error(format!("{:#}", err)),
          ));
        }
      };
      let media_type = raw_data_url.media_type();
      let data_url_text = match raw_data_url.decode() {
        Ok(text) => text,
        Err(err) => {
          return deno_core::ModuleLoadResponse::Sync(Err(
            JsErrorBox::type_error(format!("{:#}", err)),
          ));
        }
      };
      // Transpile when the data URL carries TypeScript/JSX, so compiled
      // programs can dynamically import TypeScript built at runtime.
      let code = match transpile_runtime_module(
        original_specifier,
        media_type,
        data_url_text,
      ) {
        Ok(code) => code,
        Err(err) => {
          return deno_core::ModuleLoadResponse::Sync(Err(err));
        }
      };
      return deno_core::ModuleLoadResponse::Sync(Ok(
        deno_core::ModuleSource::new(
          deno_core::ModuleType::JavaScript,
          ModuleSourceCode::String(code.into()),
          original_specifier,
          None,
        ),
      ));
    }

    if original_specifier.scheme() == "blob" {
      let specifier = original_specifier.clone();
      // Prefer the synchronously-captured root blob (if this is the worker's
      // main module) so a racing `URL.revokeObjectURL` can't make the load
      // fail; otherwise look the object URL up in the blob store.
      let maybe_blob = self
        .maybe_main_module_blob
        .as_ref()
        .filter(|(blob_specifier, _)| *blob_specifier == specifier)
        .map(|(_, blob)| blob.clone())
        .or_else(|| self.shared.blob_store.get_object_url(specifier.clone()));
      let Some(blob) = maybe_blob else {
        return deno_core::ModuleLoadResponse::Sync(Err(
          JsErrorBox::type_error(format!("Blob URL not found: {specifier}")),
        ));
      };
      let requested_module_type = options.requested_module_type.clone();
      return deno_core::ModuleLoadResponse::Async(
        async move {
          let bytes = blob.read_all().await;
          let (media_type, maybe_charset) =
            deno_media_type::resolve_media_type_and_charset_from_content_type(
              &specifier,
              Some(&blob.media_type),
            );
          let module_type = module_type_from_media_and_requested_type(
            media_type,
            &requested_module_type,
          );
          let source = match module_type {
            ModuleType::Bytes | ModuleType::Wasm => {
              ModuleSourceCode::Bytes(bytes.into_boxed_slice().into())
            }
            _ => {
              // Mirror `deno run`'s charset-aware decoding (see
              // `TextDecodedFile::decode` in cli/file_fetcher.rs) so blob
              // sources honor the content-type charset and have a leading BOM
              // stripped, instead of blindly assuming UTF-8.
              let charset = maybe_charset.unwrap_or_else(|| {
                deno_media_type::encoding::detect_charset(&specifier, &bytes)
              });
              let text =
                deno_media_type::encoding::decode_owned_source(charset, bytes)
                  .map_err(|err| {
                    JsErrorBox::type_error(format!(
                      "Failed decoding \"{specifier}\": {err}"
                    ))
                  })?;
              // Transpile when the blob carries TypeScript/JSX, so compiled
              // programs can dynamically import TypeScript built at runtime.
              let text =
                transpile_runtime_module(&specifier, media_type, text)?;
              ModuleSourceCode::String(text.into())
            }
          };
          Ok(deno_core::ModuleSource::new(
            module_type,
            source,
            &specifier,
            None,
          ))
        }
        .boxed_local(),
      );
    }

    // When load hooks are active, delegate to JS hooks first.
    // Only route through hooks for files that are NOT embedded in the binary
    // (i.e., external files that may need transformation like TS stripping).
    // Embedded files load directly from VFS without needing hooks.
    let is_embedded = if original_specifier.scheme() == "file" {
      deno_path_util::url_to_file_path(original_specifier)
        .map(|path| self.shared.vfs.file_entry(&path).is_ok())
        .unwrap_or(false)
    } else {
      // Remote modules in the binary are always embedded
      self
        .shared
        .modules
        .resolve_specifier(original_specifier)
        .ok()
        .flatten()
        .is_some()
    };
    if self.hook_registry.load_active.get()
      && !options.is_synchronous
      && !is_embedded
    {
      // The hook function itself is synchronous, but this load path has no V8
      // scope. The async bridge lets the JS event loop run the hook; blocking
      // here waiting for JS would deadlock the same runtime.
      let import_attributes = hook_load_import_attributes(
        &self.hook_registry,
        original_specifier.as_str(),
        &options.requested_module_type,
      );
      let receiver = self
        .hook_registry
        .push_load(original_specifier.to_string(), import_attributes);
      let this = self.clone();
      let specifier = original_specifier.clone();
      let maybe_referrer = maybe_referrer.cloned();
      let is_dynamic_import = options.is_dynamic_import;
      let is_synchronous = options.is_synchronous;
      let requested_module_type = options.requested_module_type.clone();
      return deno_core::ModuleLoadResponse::Async(
        async move {
          let hook_result = match receiver.await {
            Ok(r) => r,
            Err(_) => {
              return Err(JsErrorBox::generic("module load hook cancelled"));
            }
          };
          match hook_result {
            Ok((Some(source), format, _effective_url)) => {
              // Hook provided transformed source. Honor a hook-supplied
              // `format` (Node's hook contract) first, then fall back to
              // the importer's `with { type: "..." }` attribute, so e.g.
              // a passthrough hook on a JSON import still parses as JSON.
              let module_type = pick_hook_module_type(
                format.as_deref(),
                &requested_module_type,
              );
              Ok(deno_core::ModuleSource::new(
                module_type,
                ModuleSourceCode::String(source.into()),
                &specifier,
                None,
              ))
            }
            Ok((None, _, effective_url)) => {
              // Fallthrough: hooks didn't intercept; route through the same
              // default loader that an un-hooked load would use, so npm
              // packages and embedded modules resolve correctly. If the
              // user's load hook chain delegated via `nextLoad(newUrl)`,
              // fetch source from that URL while keeping the module's
              // identity at the original specifier (matches Node).
              let fetch_specifier = effective_url
                .as_deref()
                .and_then(|u| deno_core::ModuleSpecifier::parse(u).ok())
                .filter(|u| u != &specifier);
              let load_url = fetch_specifier.as_ref().unwrap_or(&specifier);
              let response = this.load_default(
                load_url,
                maybe_referrer.as_ref(),
                ModuleLoadOptions {
                  is_dynamic_import,
                  is_synchronous,
                  requested_module_type,
                },
              );
              let source = match response {
                deno_core::ModuleLoadResponse::Sync(r) => r?,
                deno_core::ModuleLoadResponse::Async(fut) => fut.await?,
              };
              if fetch_specifier.is_some() {
                Ok(deno_core::ModuleSource::new(
                  source.module_type,
                  source.code,
                  &specifier,
                  source.code_cache,
                ))
              } else {
                Ok(source)
              }
            }
            Err(err) => Err(JsErrorBox::generic(err)),
          }
        }
        .boxed_local(),
      );
    }

    self.load_default(original_specifier, maybe_referrer, options)
  }

  fn get_code_cache(
    &self,
    specifier: &ModuleSpecifier,
    source: &deno_core::v8::String,
  ) -> Option<SourceCodeCacheInfo> {
    let code_cache = self.shared.code_cache.as_ref()?;
    if !code_cache.enabled() {
      return None;
    }
    // Residual ext-scripts (the node polyfill closure, etc.) don't go through
    // `load()`, so they can't ride their cache on a `ModuleSource`. Read the
    // shared on-disk cache directly here. The deno version is already part of
    // the root cache key. Hash the `v8::String` the same way `CliModuleLoader`
    // does so this read side lines up with the `code_cache_ready` write side.
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

  fn get_source_map(&self, file_name: &str) -> Option<Cow<'_, [u8]>> {
    let url = Url::parse(file_name).ok()?;
    let data = self.shared.modules.read(&url).ok()??;
    data.source_map
  }

  fn load_external_source_map(
    &self,
    source_map_url: &str,
  ) -> Option<Cow<'_, [u8]>> {
    let url = Url::parse(source_map_url).ok()?;
    let data = self.shared.modules.read(&url).ok()??;
    Some(Cow::Owned(data.data.to_vec()))
  }

  fn source_map_source_exists(&self, source_url: &str) -> Option<bool> {
    use sys_traits::FsMetadata;
    let specifier = Url::parse(source_url).ok()?;
    // only bother checking this for npm packages that might depend on this
    if self.shared.node_resolver.in_npm_package(&specifier)
      && let Ok(path) = deno_path_util::url_to_file_path(&specifier)
    {
      return self.sys.fs_is_file(path).ok();
    }

    Some(true)
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
        crate::colors::yellow("Warning"),
        line_number + 1,
      ))
    } else {
      Some(lines[line_number].to_string())
    }
  }
}

impl NodeRequireLoader for EmbeddedModuleLoader {
  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut PermissionsContainer,
    path: Cow<'a, Path>,
  ) -> Result<Cow<'a, Path>, JsErrorBox> {
    if self.shared.modules.path_in_root(&path) {
      // allow reading if the file is in the root directory
      return Ok(path);
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
  ) -> Result<FastString, JsErrorBox> {
    match self.shared.vfs.file_entry(path) {
      Ok(file_entry) => {
        let file_bytes = self
          .shared
          .vfs
          .read_file_offset_with_len(
            file_entry.transpiled_offset.unwrap_or(file_entry.offset),
          )
          .map_err(JsErrorBox::from_err)?;
        Ok(match from_utf8_lossy_cow(file_bytes) {
          Cow::Borrowed(s) => FastString::from_static(s),
          Cow::Owned(s) => s.into(),
        })
      }
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        // Fall back to the host filesystem. Mirrors the fallback
        // `StandaloneModules::read` does for the ESM loader path.
        // Without it, a CJS `require()` that resolves to a file
        // outside the embedded VFS (HMR / dev mode, or a dynamic
        // import that landed on a host-FS path) would fail with
        // "path not found" even though the file exists on disk and
        // the permission layer already allowed reading it.
        use sys_traits::FsRead;
        #[allow(
          clippy::disallowed_types,
          reason = "use real file system because not in vfs"
        )]
        let bytes = sys_traits::impls::RealSys
          .fs_read(path)
          .map_err(JsErrorBox::from_err)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        // Mirror the ESM loader path: TypeScript/JSX read from disk at
        // runtime hasn't been transpiled at compile time, so transpile
        // here. Non-TS media types are returned verbatim.
        let specifier = deno_path_util::url_from_file_path(path)
          .map_err(JsErrorBox::from_err)?;
        let media_type = MediaType::from_specifier(&specifier);
        let text = transpile_runtime_module(&specifier, media_type, text)?;
        Ok(text.into())
      }
      Err(err) => Err(JsErrorBox::from_err(err)),
    }
  }

  fn is_maybe_cjs(
    &self,
    specifier: &Url,
  ) -> Result<bool, PackageJsonLoadError> {
    let media_type = MediaType::from_specifier(specifier);
    self.shared.cjs_tracker.is_maybe_cjs(specifier, media_type)
  }

  fn is_maybe_cjs_from_require(
    &self,
    specifier: &Url,
  ) -> Result<bool, PackageJsonLoadError> {
    let media_type = MediaType::from_specifier(specifier);
    self
      .shared
      .cjs_tracker
      .is_maybe_cjs_from_require(specifier, media_type)
  }
}

struct StandaloneModuleLoaderFactory {
  shared: Arc<SharedModuleLoaderState>,
  sys: DenoRtSys,
}

impl StandaloneModuleLoaderFactory {
  pub fn create_result(
    &self,
    maybe_main_module_blob: Option<(ModuleSpecifier, Arc<Blob>)>,
  ) -> CreateModuleLoaderResult {
    let hook_registry = LoaderHookRegistry::default();
    let loader = Rc::new(EmbeddedModuleLoader {
      shared: self.shared.clone(),
      hook_registry: hook_registry.clone(),
      sys: self.sys.clone(),
      maybe_main_module_blob,
    });
    {
      let loader = loader.clone();
      hook_registry.set_default_resolve(Rc::new(
        move |specifier: &str, referrer: &str| {
          loader
            .resolve_inner(specifier, referrer, ResolutionKind::Import)
            .map(|s| s.to_string())
            .map_err(|e| JsErrorBox::generic(e.to_string()))
        },
      ));
    }
    CreateModuleLoaderResult {
      module_loader: loader.clone(),
      node_require_loader: loader,
      hook_registry: Some(hook_registry),
    }
  }
}

impl ModuleLoaderFactory for StandaloneModuleLoaderFactory {
  fn create_for_main(
    &self,
    _root_permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    self.create_result(None)
  }

  fn create_for_worker(
    &self,
    _parent_permissions: PermissionsContainer,
    _permissions: PermissionsContainer,
    maybe_main_module_blob: Option<(ModuleSpecifier, Arc<Blob>)>,
  ) -> CreateModuleLoaderResult {
    self.create_result(maybe_main_module_blob)
  }
}

struct StandaloneRootCertStoreProvider {
  sys: DenoRtSys,
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
        get_root_cert_store(
          &self.sys,
          None,
          self.ca_stores.clone(),
          self.ca_data.clone(),
        )
      })
      .as_ref()
      .map_err(|err| JsErrorBox::from_err(err.clone()))
  }
}

/// Callback to initialize additional `OpState` during worker creation.
pub type OpStateInitFn = Box<dyn FnOnce(&mut deno_core::OpState) + Send>;

/// Options to override default standalone runtime behavior.
/// Used by the desktop runtime to enable auto_serve and set the port.
#[derive(Default)]
pub struct RunOptions {
  pub auto_serve: bool,
  pub serve_port: Option<u16>,
  pub serve_host: Option<String>,
  /// Enable HMR file watching from this directory.
  pub hmr_watch_dir: Option<PathBuf>,
  /// Callback invoked after each successful HMR replacement.
  pub hmr_on_reload: Option<crate::hmr::HmrReloadCallback>,
  /// Callback to initialize additional OpState (e.g. desktop APIs).
  /// Called during worker creation to inject state without adding extensions.
  pub op_state_init: Option<OpStateInitFn>,
  /// Override the main module to execute instead of the embedded entrypoint.
  /// Used by desktop runtime for forked worker processes (child_process.fork)
  /// where the child should run a specific script, not the embedded entrypoint.
  pub override_main_module: Option<deno_core::url::Url>,
  /// Version and rollback state for desktop auto-update JS initialization.
  pub auto_update_version: Option<String>,
  pub auto_update_rolled_back: bool,
  /// Error reporting URL from deno.json `desktop.errorReporting.url`.
  pub error_reporting_url: Option<String>,
  /// Auto-update release base URL from deno.json `desktop.release.baseUrl`.
  /// Used as the default `url` for `Deno.autoUpdate` when none is passed.
  pub release_base_url: Option<String>,
  /// Stop on the first line of user code. Mirrors `--inspect-brk`.
  pub inspect_brk: bool,
  /// Wait for a DevTools session to attach before running user code.
  /// Mirrors `--inspect-wait`.
  pub inspect_wait: bool,
  /// Enables inspector-related setup in the worker (registers the runtime
  /// with the global inspector server). Mirrors `--inspect`/`-brk`/`-wait`.
  pub is_inspecting: bool,
}

/// Read NODE_CHANNEL_FD from the environment to initialize IPC for
/// forked child processes (e.g. child_process.fork()).
pub async fn run(
  fs: Arc<dyn FileSystem>,
  sys: DenoRtSys,
  data: StandaloneData,
) -> Result<i32, AnyError> {
  run_with_options(fs, sys, data, RunOptions::default()).await
}

pub async fn run_with_options(
  fs: Arc<dyn FileSystem>,
  sys: DenoRtSys,
  data: StandaloneData,
  options: RunOptions,
) -> Result<i32, AnyError> {
  let hmr_watch_dir = options.hmr_watch_dir;
  let hmr_on_reload = options.hmr_on_reload;
  let op_state_init = options.op_state_init;
  let override_main_module = options.override_main_module;
  let StandaloneData {
    metadata,
    modules,
    npm_snapshot,
    root_path,
    vfs,
  } = data;

  let root_cert_store_provider = Arc::new(StandaloneRootCertStoreProvider {
    sys: sys.clone(),
    ca_stores: metadata.ca_stores,
    ca_data: metadata.ca_data.map(CaData::Bytes),
    cell: Default::default(),
  });
  // use a dummy npm registry url
  let npm_registry_url = Url::parse("https://localhost/").unwrap();
  let root_dir_url = Arc::new(Url::from_directory_path(&root_path).unwrap());
  let workspace_root_path =
    hmr_watch_dir.clone().unwrap_or_else(|| root_path.clone());
  let workspace_root_dir_url =
    Arc::new(Url::from_directory_path(&workspace_root_path).unwrap());
  let entrypoint = root_dir_url.join(&metadata.entrypoint_key).unwrap();
  // When this process was spawned by node:child_process.fork() from a compiled
  // binary, the parent asks us to run a specific embedded module instead of the
  // baked-in entrypoint (see ext/node/polyfills/child_process.ts). Otherwise a
  // fork() would just re-run the parent's entrypoint (issue #26304).
  let child_entrypoint = std::env::var(INTERNAL_CHILD_ENTRYPOINT_ENV_VAR)
    .ok()
    .filter(|module_path| !module_path.is_empty());
  if child_entrypoint.is_some() {
    // Always strip the internal var so it does not leak into any grandchild
    // processes that inherit this environment, even if we end up ignoring it.
    // SAFETY: single-threaded during startup, before the runtime is created.
    unsafe { std::env::remove_var(INTERNAL_CHILD_ENTRYPOINT_ENV_VAR) };
  }
  let main_module = if let Some(url) = override_main_module {
    url
  } else {
    match child_entrypoint {
      // Only honor the override for a genuine fork() child, which always wires
      // up an IPC channel (NODE_CHANNEL_FD). This is defense in depth against an
      // accidental collision with a stray DENO_INTERNAL_CHILD_ENTRYPOINT, not a
      // security boundary: an attacker who can set one env var can set both, and
      // resolve_child_entrypoint's cwd-relative fallback will then run an
      // on-disk module with the binary's baked-in permissions. That on-disk
      // fallback is intentional (it matches fork() semantics outside a compiled
      // binary), so a hostile environment is out of scope here. NODE_CHANNEL_FD
      // is still present because node_ipc_init() consumes it later.
      Some(module_path) if std::env::var("NODE_CHANNEL_FD").is_ok() => {
        resolve_child_entrypoint(&module_path, &entrypoint, &vfs, &sys)
      }
      _ => entrypoint,
    }
  };
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
        default_config: deno_npmrc::RegistryConfigWithUrl {
          registry_url: npm_registry_url.clone(),
          config: Default::default(),
        },
        scopes: Default::default(),
        registry_configs: Default::default(),
        min_release_age_days: None,
        trust_policy: Default::default(),
        trust_policy_ignore_after_minutes: None,
        trust_policy_exclude: Vec::new(),
      });
      let npm_cache_dir = Arc::new(NpmCacheDir::new(
        &sys,
        npm_global_cache_dir,
        npmrc.get_all_known_registries_urls(),
      ));
      let snapshot = npm_snapshot.unwrap();
      let maybe_node_modules_path = node_modules_dir
        .map(|node_modules_dir| workspace_root_path.join(node_modules_dir));
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
          sys: node_resolution_sys.clone(),
          maybe_node_modules_path,
          npm_system_info: Default::default(),
          npmrc,
          linker_mode: deno_config::deno_json::NodeModulesLinkerMode::default(),
        }),
      );
      (in_npm_pkg_checker, npm_resolver)
    }
    Some(NodeModules::Byonm {
      root_node_modules_dir,
    }) => {
      let root_node_modules_dir =
        root_node_modules_dir.map(|p| workspace_root_path.join(p));
      let in_npm_pkg_checker =
        DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);
      let npm_resolver = NpmResolver::<DenoRtSys>::new::<DenoRtSys>(
        NpmResolverCreateOptions::Byonm(ByonmNpmResolverCreateOptions {
          sys: node_resolution_sys.clone(),
          pkg_json_resolver: pkg_json_resolver.clone(),
          root_node_modules_dir,
          search_stop_dir: Some(workspace_root_path.clone()),
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
          sys: node_resolution_sys.clone(),
          npm_cache_dir,
          maybe_node_modules_path: None,
          npm_system_info: Default::default(),
          npmrc: create_default_npmrc(),
          linker_mode: deno_config::deno_json::NodeModulesLinkerMode::default(),
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
  let require_modules = metadata
    .require_modules
    .iter()
    .map(|key| root_dir_url.join(key).unwrap())
    .collect::<Vec<_>>();
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
    require_modules.clone(),
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
  let node_code_translator = Arc::new(NodeCodeTranslator::new(
    cjs_module_export_analyzer,
    node_resolver::analyze::NodeCodeTranslatorMode::ModuleLoader,
  ));
  let workspace_resolver = {
    let import_map = match metadata.workspace_resolver.import_map {
      Some(import_map) => Some(
        import_map::parse_from_json_with_options(
          workspace_root_dir_url.join(&import_map.specifier).unwrap(),
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
        let path = workspace_root_dir_url
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
      workspace_root_dir_url.clone(),
      import_map,
      metadata
        .workspace_resolver
        .jsr_pkgs
        .iter()
        .map(|pkg| ResolverWorkspaceJsrPackage {
          is_link: false, // only used for enhancing the diagnostic, which isn't shown in deno compile
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
        SloppyImportsOptions::Unspecified
      },
      Default::default(),
      sys.clone(),
      metadata.workspace_resolver.catalogs,
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
  let blob_store = Arc::new(BlobStore::default());
  let module_loader_factory = StandaloneModuleLoaderFactory {
    shared: Arc::new(SharedModuleLoaderState {
      blob_store: blob_store.clone(),
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
    sys: sys.clone(),
  };

  // Desktop runtimes always need a baseline set of permissions to function:
  // the renderer is served over loopback HTTP, so net access to 127.0.0.1
  // is mandatory; the desktop init JS reads `DENO_DESKTOP_*` env vars to
  // discover the inspector mux, HMR watch dir, etc. Without these, the
  // very first `Deno.serve(...)` or `Deno.env.get(...)` throws NotCapable
  // and aborts script execution before the event-loop IIFE registers —
  // which manifests as a blank window where nothing works (no input,
  // no auto-tests, no bindings). Granting them up front avoids forcing
  // every `deno desktop` user to remember `-A`.
  let has_desktop = op_state_init.is_some();
  let permissions = {
    let mut permissions = metadata.permissions;
    // grant read access to the vfs
    grant_vfs_read_access(&mut permissions, &root_path);

    if has_desktop {
      apply_desktop_permission_defaults(&mut permissions);
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
  // Resolve a persistent, per-app data directory so origin-bound storage such
  // as the default `Deno.openKv()`, `localStorage` and `caches` persists to
  // disk instead of silently falling back to an in-memory database. A compiled
  // binary is a standalone app, so we store under the platform's app data
  // directory (`%LOCALAPPDATA%`, `~/Library/Application Support`,
  // `$XDG_DATA_HOME`) keyed by the app name, rather than polluting `DENO_DIR`.
  //
  // The app name is baked in at compile time (`--app-name`, otherwise the
  // output file name), so it is stable across runs and even if the binary is
  // later renamed. Recompiling with a different `--output`/`--app-name` starts
  // a fresh store. If we can't resolve a data directory we leave both pieces
  // unset and keep the in-memory fallback.
  //
  // `metadata.app_name` is always set by binaries this version produces; the
  // fallback to the executable file stem only matters for binaries compiled
  // before the field existed.
  let app_name = metadata.app_name.unwrap_or_else(|| {
    std::env::current_exe()
      .ok()
      .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
      .unwrap_or_else(|| "deno-compile".to_string())
  });
  // `--app-name` is validated at compile time (see `validate_app_name`), so it
  // is safe to use directly as a single directory component here.
  let origin_data_folder_path =
    crate::binary::get_data_local_dir().map(|dir| dir.join(&app_name));
  // Only enable persistent storage when we resolved a data directory. The
  // storage key resolver must stay empty otherwise, because the worker unwraps
  // `origin_data_folder_path` whenever the key resolves to a value.
  let storage_key_resolver = if origin_data_folder_path.is_some() {
    StorageKeyResolver::from_compile_app_name(&app_name)
  } else {
    StorageKeyResolver::empty()
  };
  let lib_main_worker_options = LibMainWorkerOptions {
    argv: metadata.argv,
    log_level: WorkerLogLevel::Info,
    enable_testing_features: false,
    has_node_modules_dir,
    inspect_brk: options.inspect_brk,
    inspect_wait: options.inspect_wait,
    trace_ops: None,
    is_inspecting: options.is_inspecting,
    is_standalone: true,
    auto_serve: options.auto_serve,
    skip_op_registration: true,
    location: metadata.location,
    argv0: NpmPackageReqReference::from_specifier(&main_module)
      .ok()
      .map(|req_ref| npm_pkg_req_ref_to_binary_command(&req_ref).to_string())
      .or(std::env::args().next()),
    node_debug: std::env::var("NODE_DEBUG").ok(),
    node_cluster_unique_id: std::env::var("NODE_UNIQUE_ID").ok(),
    node_cluster_sched_policy: std::env::var("NODE_CLUSTER_SCHED_POLICY").ok(),
    origin_data_folder_path,
    seed: metadata.seed,
    unsafely_ignore_certificate_errors: metadata
      .unsafely_ignore_certificate_errors,
    node_ipc_init: deno_lib::args::node_ipc_init(&sys)?,
    serve_port: options.serve_port,
    serve_host: options.serve_host,
    otel_config: metadata.otel_config,
    no_legacy_abort: false,
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    residual_lazy_js_sources: deno_snapshots::RESIDUAL_LAZY_JS,
    residual_lazy_esm_sources: deno_snapshots::RESIDUAL_LAZY_ESM,
    enable_raw_imports: metadata.unstable_config.raw_imports,
    close_on_idle: !options.auto_serve,
    maybe_initial_cwd: None,
    disable_offscreen_canvas: matches!(
      std::env::var("DENO_DISABLE_OFFSCREEN_CANVAS").as_deref(),
      Ok("1") | Ok("true")
    ),
  };
  let worker_factory = LibMainWorkerFactory::new(
    blob_store,
    code_cache.map(|c| c.for_deno_core()),
    sys.maybe_native_addon_loader(),
    feature_checker,
    fs,
    None, // maybe_coverage_dir
    None, // maybe_cpu_prof_config
    Box::new(module_loader_factory),
    node_resolver.clone(),
    create_npm_process_state_provider(&npm_resolver),
    pkg_json_resolver,
    root_cert_store_provider,
    storage_key_resolver,
    sys.clone(),
    lib_main_worker_options,
    Default::default(),
    None,
  );

  // Initialize v8 once from the main thread.
  v8_set_flags(construct_v8_flags(&[], &metadata.v8_flags, vec![]));
  let is_single_threaded = metadata
    .v8_flags
    .contains(&String::from("--single-threaded"));
  let v8_platform = if is_single_threaded {
    Some(::deno_core::v8::Platform::new_single_threaded(true).make_shared())
  } else {
    None
  };
  deno_core::JsRuntime::init_platform(v8_platform);

  // On Windows, ICU detects the host time zone from the OS and ignores the
  // `TZ` environment variable. Apply it to ICU explicitly before any isolate
  // uses `Date`. On unix ICU already honors `TZ`.
  #[cfg(windows)]
  if let Some(tz) = std::env::var_os("TZ")
    .and_then(|tz| tz.into_string().ok())
    .filter(|tz| !tz.is_empty())
  {
    deno_core::v8::icu::set_default_time_zone(&tz);
  }

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

  let preload_modules = metadata
    .preload_modules
    .iter()
    .map(|key| root_dir_url.join(key).unwrap())
    .collect::<Vec<_>>();

  let mut worker = worker_factory.create_custom_worker(
    WorkerExecutionMode::Run,
    main_module,
    preload_modules,
    require_modules,
    permissions,
    vec![],
    Default::default(),
    None,
  )?;

  // Inject desktop API state into OpState after worker creation.
  if let Some(init_fn) = op_state_init {
    let op_state = worker.js_runtime().op_state();
    init_fn(&mut op_state.borrow_mut());
  }

  // Initialize desktop APIs (Deno.desktop.*).
  if has_desktop {
    worker
      .js_runtime()
      .execute_script("ext:deno_desktop/init", crate::desktop::DESKTOP_JS)?;

    // Initialize auto-update JS (DesktopUpdater cppgc object is already
    // registered via the desktop extension's objects).
    let js = crate::desktop::desktop_auto_update_js(
      options.auto_update_version.as_deref(),
      options.auto_update_rolled_back,
      options.release_base_url.as_deref(),
    );
    worker
      .js_runtime()
      .execute_script("ext:deno_desktop/auto_update", js)?;

    let js = crate::desktop::desktop_error_reporting_js(
      options.error_reporting_url.as_deref(),
      options.auto_update_version.as_deref(),
    );
    worker
      .js_runtime()
      .execute_script("ext:deno_desktop/error_reporting", js)?;
  }

  if let Some(watch_dir) = hmr_watch_dir {
    let mut hmr_runner =
      crate::hmr::setup_desktop_hmr(&mut worker, watch_dir, root_path.clone())?;
    if let Some(cb) = hmr_on_reload {
      hmr_runner.set_on_reload(cb);
    }

    // Run one event loop tick so the inspector processes the
    // Debugger.enable / Runtime.enable messages before we load modules.
    // This ensures scriptParsed notifications are emitted during module load.
    worker.run_event_loop(false).await?;

    // Run preload modules first
    worker.execute_preload_modules().await?;
    worker.execute_main_module().await?;
    worker.dispatch_load_event()?;

    loop {
      let hmr_fut = hmr_runner.run();
      let event_loop_fut = worker.run_event_loop(false);

      tokio::select! {
        hmr_result = hmr_fut => {
          if let Err(e) = hmr_result {
            log::error!("HMR error: {:?}", e);
          }
        }
        event_loop_result = event_loop_fut => {
          event_loop_result?;
          let web_continue = worker.dispatch_beforeunload_event()?;
          if !web_continue {
            let node_continue =
              worker.dispatch_process_beforeexit_event()?;
            if !node_continue {
              break;
            }
          }
        }
      }
    }

    worker.dispatch_unload_event()?;
    worker.dispatch_process_exit_event()?;
    Ok(worker.exit_code())
  } else {
    let exit_code = worker.run().await?;
    Ok(exit_code)
  }
}

// Internal env var carrying the module a fork()ed child of a compiled binary
// should run. Kept in sync with ext/node/polyfills/child_process.ts.
const INTERNAL_CHILD_ENTRYPOINT_ENV_VAR: &str =
  "DENO_INTERNAL_CHILD_ENTRYPOINT";

/// Resolves the module path passed to `node:child_process.fork()` to the module
/// that a compiled-binary child should run as its main module.
///
/// A relative path is first resolved against the parent entrypoint's directory
/// so it points at a sibling module embedded in the compile VFS; if no such
/// module is embedded there, it falls back to a path relative to the real cwd
/// so an on-disk module can still be loaded (matching how fork() resolves a
/// relative path outside a compiled binary). Absolute paths are used as-is: the
/// module loader consults the VFS first and falls back to disk. Falls back to
/// the entrypoint if resolution fails entirely.
fn resolve_child_entrypoint(
  module_path: &str,
  entrypoint: &Url,
  vfs: &FileBackedVfs,
  sys: &DenoRtSys,
) -> Url {
  // Falling back to the entrypoint silently would re-run the parent's
  // entrypoint, i.e. the very symptom of #26304. Warn so the failure is
  // diagnosable instead of looking like a successful fork.
  //
  // Re-running the entrypoint risks a re-fork loop if the entrypoint
  // unconditionally fork()s the same bad path. In practice this almost never
  // fires: the common "missing module" case takes the cwd branch below, where
  // resolve_path() builds a URL without an existence check, so the bad path
  // surfaces as a module load error rather than reaching here. fallback() only
  // triggers on an unparseable path or an unreadable cwd, which a re-fork would
  // hit identically (and thus surface) rather than spinning silently.
  let fallback = || {
    log::warn!(
      "Could not resolve module {:?} passed to child_process.fork(); running the compiled entrypoint instead.",
      module_path
    );
    entrypoint.clone()
  };
  let path = std::path::Path::new(module_path);
  if path.is_absolute() {
    return deno_path_util::url_from_file_path(path)
      .unwrap_or_else(|_| fallback());
  }
  // Prefer the sibling module embedded next to the entrypoint in the VFS.
  if let Ok(candidate) = entrypoint.join(module_path)
    && let Ok(candidate_path) = deno_path_util::url_to_file_path(&candidate)
    && vfs.stat(&candidate_path).is_ok()
  {
    return candidate;
  }
  // Otherwise resolve against the real cwd to load an on-disk module. fork()
  // resolves a relative module path against the process's current working
  // directory, so reading the real cwd is the intended behavior here even
  // though the lint discourages ambient cwd access elsewhere.
  #[allow(
    clippy::disallowed_methods,
    reason = "fork() resolves relative module paths against the process cwd"
  )]
  let cwd = sys.env_current_dir();
  match cwd {
    Ok(cwd) => {
      deno_core::resolve_path(module_path, &cwd).unwrap_or_else(|_| fallback())
    }
    Err(_) => fallback(),
  }
}

/// Build the `importAttributes` object handed to a `module.registerHooks()`
/// load hook. V8 only surfaces the `type` attribute at load time, so recover
/// the full `with { ... }` clause recorded during resolution and make sure
/// `type` is present for the common JSON/text/bytes case.
fn hook_load_import_attributes(
  registry: &LoaderHookRegistry,
  specifier: &str,
  requested_module_type: &RequestedModuleType,
) -> std::collections::HashMap<String, String> {
  let mut attrs = registry.take_resolved_attributes(specifier);
  if let Some(ty) = requested_module_type.as_str() {
    attrs
      .entry("type".to_string())
      .or_insert_with(|| ty.to_string());
  }
  attrs
}

/// Pick the `ModuleType` for source returned by a `module.registerHooks()`
/// load hook. Honors a hook-supplied `format` first (Node's hook contract),
/// then falls back to the importer's `with { type: "..." }` attribute.
fn pick_hook_module_type(
  format: Option<&str>,
  requested: &deno_core::RequestedModuleType,
) -> deno_core::ModuleType {
  if let Some(format) = format {
    match format {
      "json" => return deno_core::ModuleType::Json,
      "text" => return deno_core::ModuleType::Text,
      "bytes" => return deno_core::ModuleType::Bytes,
      "wasm" => return deno_core::ModuleType::Wasm,
      _ => {}
    }
  }
  match requested {
    deno_core::RequestedModuleType::Json => deno_core::ModuleType::Json,
    deno_core::RequestedModuleType::Text => deno_core::ModuleType::Text,
    deno_core::RequestedModuleType::Bytes => deno_core::ModuleType::Bytes,
    _ => deno_core::ModuleType::JavaScript,
  }
}

fn create_default_npmrc() -> Arc<ResolvedNpmRc> {
  // this is fine because multiple registries are combined into
  // one when compiling the binary
  Arc::new(ResolvedNpmRc {
    default_config: deno_npmrc::RegistryConfigWithUrl {
      registry_url: Url::parse(deno_npmrc::NPM_DEFAULT_REGISTRY).unwrap(),
      config: Default::default(),
    },
    scopes: Default::default(),
    registry_configs: Default::default(),
    min_release_age_days: None,
    trust_policy: Default::default(),
    trust_policy_ignore_after_minutes: None,
    trust_policy_exclude: Vec::new(),
  })
}

/// Grant read access to the embedded VFS root so the compiled binary
/// can read the files it shipped with. Preserves the `Some(vec![])`
/// allow-all sentinel and avoids duplicating an already-present entry.
pub(crate) fn grant_vfs_read_access(
  permissions: &mut deno_runtime::deno_permissions::PermissionsOptions,
  root_path: &std::path::Path,
) {
  let entry = root_path.to_string_lossy().into_owned();
  match &mut permissions.allow_read {
    Some(vec) if vec.is_empty() => {
      // already wide-open (--allow-read with no args / -A); nothing to
      // extend.
    }
    Some(vec) => {
      if !vec.contains(&entry) {
        vec.push(entry);
      }
    }
    None => {
      permissions.allow_read = Some(vec![entry]);
    }
  }
}

/// Loopback hosts a desktop runtime must be able to reach. The compiled
/// app's renderer talks to its `Deno.serve()` over loopback HTTP, so
/// net access here is structural, not optional.
const DESKTOP_LOOPBACK_HOSTS: &[&str] = &["127.0.0.1", "localhost", "[::1]"];

/// Env vars the DESKTOP_JS init script reads. Granting only these
/// (instead of blanket `allow-env`) keeps the user's shell environment
/// off-limits to arbitrary `Deno.env.get` calls in their app code while
/// still letting the framework boot.
const DESKTOP_ENV_KEYS: &[&str] = &[
  "DENO_DESKTOP_MUX_WS",
  "DENO_DESKTOP_HMR",
  "DENO_DESKTOP_INSPECT_INTERNAL_PORT",
  "DENO_DESKTOP_INSPECT_BRK",
  "DENO_DESKTOP_INSPECT_WAIT",
  "LAUFEY_RUNTIME_PATH",
  "LAUFEY_REMOTE_DEBUGGING_PORT",
];

/// Grant the baseline net (loopback) and env (DESKTOP_JS read keys)
/// access a desktop runtime needs to boot. Idempotent: re-applying
/// preserves any wider grants the user already had.
///
/// `Some(vec)` with `vec.is_empty()` represents allow-all (e.g. `-A`)
/// and is left untouched. `Some(vec)` with entries gets the missing
/// loopback/key entries appended. `None` is replaced with the baseline
/// vec.
pub(crate) fn apply_desktop_permission_defaults(
  permissions: &mut deno_runtime::deno_permissions::PermissionsOptions,
) {
  fn extend(slot: &mut Option<Vec<String>>, defaults: &[&str]) {
    match slot {
      Some(vec) if vec.is_empty() => {
        // already allow-all — wider than our baseline; don't narrow.
      }
      Some(vec) => {
        for entry in defaults {
          let s = (*entry).to_string();
          if !vec.contains(&s) {
            vec.push(s);
          }
        }
      }
      None => {
        *slot = Some(defaults.iter().map(|s| (*s).to_string()).collect());
      }
    }
  }

  extend(&mut permissions.allow_net, DESKTOP_LOOPBACK_HOSTS);
  extend(&mut permissions.allow_env, DESKTOP_ENV_KEYS);
}

#[cfg(test)]
mod tests {
  use deno_runtime::deno_permissions::PermissionsOptions;

  use super::DESKTOP_ENV_KEYS;
  use super::DESKTOP_LOOPBACK_HOSTS;
  use super::apply_desktop_permission_defaults;
  use super::grant_vfs_read_access;

  fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
  }

  #[test]
  fn defaults_populate_empty_options() {
    let mut p = PermissionsOptions::default();
    apply_desktop_permission_defaults(&mut p);
    assert_eq!(p.allow_net, Some(strs(DESKTOP_LOOPBACK_HOSTS)));
    assert_eq!(p.allow_env, Some(strs(DESKTOP_ENV_KEYS)));
    // Other permission categories must remain untouched.
    assert_eq!(p.allow_read, None);
    assert_eq!(p.allow_write, None);
  }

  #[test]
  fn defaults_extend_existing_allowlist_without_duplicating() {
    let mut p = PermissionsOptions {
      allow_net: Some(vec![
        "example.com".to_string(),
        "127.0.0.1".to_string(), // already present — must not duplicate
      ]),
      allow_env: Some(vec!["MY_VAR".to_string()]),
      ..Default::default()
    };
    apply_desktop_permission_defaults(&mut p);

    let net = p.allow_net.unwrap();
    // Original entries preserved.
    assert!(net.contains(&"example.com".to_string()));
    // 127.0.0.1 still appears exactly once.
    assert_eq!(net.iter().filter(|s| *s == "127.0.0.1").count(), 1);
    // Missing loopback entries appended.
    assert!(net.contains(&"localhost".to_string()));
    assert!(net.contains(&"[::1]".to_string()));

    let env = p.allow_env.unwrap();
    assert!(env.contains(&"MY_VAR".to_string()));
    for key in DESKTOP_ENV_KEYS {
      assert!(env.contains(&key.to_string()), "missing {key}");
    }
  }

  #[test]
  fn defaults_preserve_allow_all_sentinel() {
    // `Some(vec![])` is the allow-all sentinel (-A / --allow-net with
    // no args). Narrowing it down to a specific allowlist would silently
    // break user code that depends on broader access — leave it alone.
    let mut p = PermissionsOptions {
      allow_net: Some(vec![]),
      allow_env: Some(vec![]),
      ..Default::default()
    };
    apply_desktop_permission_defaults(&mut p);
    assert_eq!(p.allow_net, Some(vec![]));
    assert_eq!(p.allow_env, Some(vec![]));
  }

  #[test]
  fn defaults_are_idempotent() {
    let mut p = PermissionsOptions::default();
    apply_desktop_permission_defaults(&mut p);
    let after_once = p.clone();
    apply_desktop_permission_defaults(&mut p);
    assert_eq!(p, after_once);
  }

  // --- grant_vfs_read_access ---

  #[test]
  fn vfs_grant_populates_empty_options() {
    let mut p = PermissionsOptions::default();
    grant_vfs_read_access(&mut p, std::path::Path::new("/data/vfs"));
    assert_eq!(p.allow_read, Some(vec!["/data/vfs".to_string()]));
    // Net / env / write must remain None.
    assert_eq!(p.allow_net, None);
    assert_eq!(p.allow_env, None);
    assert_eq!(p.allow_write, None);
  }

  #[test]
  fn vfs_grant_preserves_allow_all_sentinel() {
    // `Some(vec![])` means --allow-read with no args (allow-all). The
    // compiled binary should keep its broader privilege; narrowing it
    // to a single path here would silently break user code that reads
    // outside the VFS.
    let mut p = PermissionsOptions {
      allow_read: Some(vec![]),
      ..Default::default()
    };
    grant_vfs_read_access(&mut p, std::path::Path::new("/data/vfs"));
    assert_eq!(p.allow_read, Some(vec![]));
  }

  #[test]
  fn vfs_grant_extends_existing_allowlist() {
    let mut p = PermissionsOptions {
      allow_read: Some(vec!["/etc".to_string()]),
      ..Default::default()
    };
    grant_vfs_read_access(&mut p, std::path::Path::new("/data/vfs"));
    let v = p.allow_read.unwrap();
    assert!(
      v.contains(&"/etc".to_string()),
      "original entries preserved"
    );
    assert!(v.contains(&"/data/vfs".to_string()));
  }

  #[test]
  fn vfs_grant_is_idempotent() {
    let mut p = PermissionsOptions::default();
    grant_vfs_read_access(&mut p, std::path::Path::new("/data/vfs"));
    grant_vfs_read_access(&mut p, std::path::Path::new("/data/vfs"));
    // Same path twice must not duplicate.
    assert_eq!(p.allow_read, Some(vec!["/data/vfs".to_string()]));
  }

  #[test]
  fn vfs_grant_handles_path_with_unicode() {
    let mut p = PermissionsOptions::default();
    grant_vfs_read_access(&mut p, std::path::Path::new("/Users/café/My App"));
    assert_eq!(p.allow_read, Some(vec!["/Users/café/My App".to_string()]));
  }
}
