// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod standalone;

mod args;
mod cache;
mod denort;
mod emit;
mod errors;
mod file_fetcher;
mod http_util;
mod js;
mod node;
mod npm;
mod resolver;
mod shared;
mod sys;
mod task_runner;
mod util;
mod version;
mod worker;

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::env::current_exe;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::npm::NpmCacheDir;
use deno_config::workspace::MappedResolution;
use deno_config::workspace::MappedResolutionError;
use deno_config::workspace::ResolverWorkspaceJsrPackage;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_core::v8_set_flags;
use deno_core::FastString;
use deno_core::FeatureChecker;
use deno_core::ModuleLoader;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::SourceCodeCacheInfo;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_package_json::PackageJsonDepValue;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::npm::NpmReqResolverOptions;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::RealIsBuiltInNodeModuleChecker;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::WorkerLogLevel;
use deno_runtime::UNSTABLE_GRANULAR_FLAGS;
use deno_semver::npm::NpmPackageReqReference;
use deno_terminal::colors;
use import_map::parse_from_json;
use indexmap::IndexMap;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;

use crate::args::create_default_npmrc;
use crate::args::get_root_cert_store;
use crate::args::npm_pkg_req_ref_to_binary_command;
use crate::args::CaData;
use crate::args::NpmInstallDepsProvider;
use crate::args::StorageKeyResolver;
use crate::cache::Caches;
use crate::cache::DenoDirProvider;
use crate::cache::FastInsecureHasher;
use crate::cache::NodeAnalysisCache;
use crate::denort::code_cache::DenoCompileCodeCache;
use crate::http_util::HttpClientProvider;
use crate::node::CliCjsCodeAnalyzer;
use crate::node::CliNodeCodeTranslator;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::create_cli_npm_resolver;
use crate::npm::create_in_npm_pkg_checker;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliManagedInNpmPkgCheckerCreateOptions;
use crate::npm::CliManagedNpmResolverCreateOptions;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::npm::CreateInNpmPkgCheckerOptions;
use crate::resolver::CjsTracker;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::NpmModuleLoader;
use crate::standalone::binary::Metadata;
use crate::standalone::binary::NodeModules;
use crate::standalone::serialization::deserialize_binary_data_section;
use crate::standalone::serialization::DenoCompileModuleSource;
use crate::standalone::serialization::DeserializedDataSection;
use crate::standalone::serialization::RemoteModulesStore;
use crate::standalone::serialization::SourceMapStore;
use crate::standalone::virtual_fs::FileBackedVfs;
use crate::standalone::virtual_fs::VfsFileSubDataKind;
use crate::standalone::virtual_fs::VfsRoot;
use crate::standalone::virtual_fs::VirtualDirectory;
use crate::standalone::MODULE_NOT_FOUND;
use crate::standalone::UNSUPPORTED_SCHEME;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::text_encoding::from_utf8_lossy_cow;
use crate::util::v8::construct_v8_flags;
use crate::worker::CliCodeCache;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;
use crate::worker::CreateModuleLoaderResult;
use crate::worker::ModuleLoaderFactory;

use crate::args::Flags;

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  log::error!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  deno_runtime::exit(70);
}

fn exit_with_message(message: &str, code: i32) -> ! {
  log::error!(
    "{}: {}",
    colors::red_bold("error"),
    message.trim_start_matches("error: ")
  );
  deno_runtime::exit(code);
}

fn unwrap_or_exit<T>(result: Result<T, AnyError>) -> T {
  match result {
    Ok(value) => value,
    Err(error) => {
      let mut error_string = format!("{:?}", error);

      if let Some(e) = error.downcast_ref::<JsError>() {
        error_string = format_js_error(e);
      }

      exit_with_message(&error_string, 1);
    }
  }
}

fn load_env_vars(env_vars: &IndexMap<String, String>) {
  env_vars.iter().for_each(|env_var| {
    if env::var(env_var.0).is_err() {
      std::env::set_var(env_var.0, env_var.1);
    }
  })
}

fn main() {
  deno_runtime::deno_permissions::mark_standalone();
  let args: Vec<_> = env::args_os().collect();
  let standalone = extract_standalone(Cow::Owned(args));
  let future = async move {
    match standalone {
      Ok(Some(data)) => {
        deno_telemetry::init(crate::args::otel_runtime_config())?;
        util::logger::init(
          data.metadata.log_level,
          Some(data.metadata.otel_config.clone()),
        );
        load_env_vars(&data.metadata.env_vars_from_env_file);
        let exit_code = run_standalone(data).await?;
        deno_runtime::exit(exit_code);
      }
      Ok(None) => Ok(()),
      Err(err) => {
        util::logger::init(None, None);
        Err(err)
      }
    }
  };

  unwrap_or_exit(create_and_run_current_thread_with_maybe_metrics(future));
}

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by skipping over the trailer width at the end of the file,
/// then checking for the magic trailer string `d3n0l4nd`. If found,
/// the bundle is executed. If not, this function exits with `Ok(None)`.
fn extract_standalone(
  cli_args: Cow<Vec<OsString>>,
) -> Result<Option<StandaloneData>, AnyError> {
  let Some(data) = libsui::find_section("d3n0l4nd") else {
    return Ok(None);
  };

  let DeserializedDataSection {
    mut metadata,
    npm_snapshot,
    remote_modules,
    source_maps,
    vfs_root_entries,
    vfs_files_data,
  } = match deserialize_binary_data_section(data)? {
    Some(data_section) => data_section,
    None => return Ok(None),
  };

  let root_path = {
    let maybe_current_exe = std::env::current_exe().ok();
    let current_exe_name = maybe_current_exe
      .as_ref()
      .and_then(|p| p.file_name())
      .map(|p| p.to_string_lossy())
      // should never happen
      .unwrap_or_else(|| Cow::Borrowed("binary"));
    std::env::temp_dir().join(format!("deno-compile-{}", current_exe_name))
  };
  let cli_args = cli_args.into_owned();
  metadata.argv.reserve(cli_args.len() - 1);
  for arg in cli_args.into_iter().skip(1) {
    metadata.argv.push(arg.into_string().unwrap());
  }
  let vfs = {
    let fs_root = VfsRoot {
      dir: VirtualDirectory {
        // align the name of the directory with the root dir
        name: root_path.file_name().unwrap().to_string_lossy().to_string(),
        entries: vfs_root_entries,
      },
      root_path: root_path.clone(),
      start_file_offset: 0,
    };
    Arc::new(FileBackedVfs::new(Cow::Borrowed(vfs_files_data), fs_root))
  };
  Ok(Some(StandaloneData {
    metadata,
    modules: StandaloneModules {
      remote_modules,
      vfs: vfs.clone(),
    },
    npm_snapshot,
    root_path,
    source_maps,
    vfs,
  }))
}

struct SharedModuleLoaderState {
  cjs_tracker: Arc<CjsTracker>,
  code_cache: Option<Arc<dyn CliCodeCache>>,
  fs: Arc<dyn deno_fs::FileSystem>,
  modules: StandaloneModules,
  node_code_translator: Arc<CliNodeCodeTranslator>,
  node_resolver: Arc<CliNodeResolver>,
  npm_module_loader: Arc<NpmModuleLoader>,
  npm_req_resolver: Arc<CliNpmReqResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
  source_maps: SourceMapStore,
  vfs: Arc<FileBackedVfs>,
  workspace_resolver: WorkspaceResolver,
}

impl SharedModuleLoaderState {
  fn get_code_cache(
    &self,
    specifier: &ModuleSpecifier,
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
    let referrer_kind = if self
      .shared
      .cjs_tracker
      .is_maybe_cjs(&referrer, MediaType::from_specifier(&referrer))?
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
          )?
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
          .resolve_package_subpath_from_deno_module(
            pkg_json.dir_path(),
            sub_path.as_deref(),
            Some(&referrer),
            referrer_kind,
            NodeResolutionKind::Execution,
          )?,
      ),
      Ok(MappedResolution::PackageJson {
        dep_result,
        sub_path,
        alias,
        ..
      }) => match dep_result.as_ref().map_err(|e| AnyError::from(e.clone()))? {
        PackageJsonDepValue::Req(req) => self
          .shared
          .npm_req_resolver
          .resolve_req_with_sub_path(
            req,
            sub_path.as_deref(),
            &referrer,
            referrer_kind,
            NodeResolutionKind::Execution,
          )
          .map_err(AnyError::from),
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
              .resolve_package_subpath_from_deno_module(
                pkg_folder,
                sub_path.as_deref(),
                Some(&referrer),
                referrer_kind,
                NodeResolutionKind::Execution,
              )?,
          )
        }
      },
      Ok(MappedResolution::Normal { specifier, .. })
      | Ok(MappedResolution::ImportMap { specifier, .. }) => {
        if let Ok(reference) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          return Ok(self.shared.npm_req_resolver.resolve_req_reference(
            &reference,
            &referrer,
            referrer_kind,
            NodeResolutionKind::Execution,
          )?);
        }

        if specifier.scheme() == "jsr" {
          if let Some(specifier) =
            self.shared.modules.resolve_specifier(&specifier)?
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
        let maybe_res = self.shared.npm_req_resolver.resolve_if_for_npm_pkg(
          raw_specifier,
          &referrer,
          referrer_kind,
          NodeResolutionKind::Execution,
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
      let shared = self.shared.clone();
      let original_specifier = original_specifier.clone();
      let maybe_referrer = maybe_referrer.cloned();
      return deno_core::ModuleLoadResponse::Async(
        async move {
          let code_source = shared
            .npm_module_loader
            .load(&original_specifier, maybe_referrer.as_ref())
            .await?;
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

    match self
      .shared
      .modules
      .read(original_specifier, VfsFileSubDataKind::ModuleGraph)
    {
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
            return deno_core::ModuleLoadResponse::Sync(Err(type_error(
              format!("{:?}", err),
            )));
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
                    Cow::Owned(bytes) => Cow::Owned(
                      crate::util::text_encoding::from_utf8_lossy_owned(bytes),
                    ),
                    Cow::Borrowed(bytes) => String::from_utf8_lossy(bytes),
                  }
                }
              };
              let source = shared
                .node_code_translator
                .translate_cjs_to_esm(&module_specifier, Some(source))
                .await?;
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
      Ok(None) => deno_core::ModuleLoadResponse::Sync(Err(type_error(
        format!("{MODULE_NOT_FOUND}: {}", original_specifier),
      ))),
      Err(err) => deno_core::ModuleLoadResponse::Sync(Err(type_error(
        format!("{:?}", err),
      ))),
    }
  }

  fn code_cache_ready(
    &self,
    specifier: ModuleSpecifier,
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
    if file_name.starts_with("file:///") {
      let url =
        deno_path_util::url_from_directory_path(self.shared.vfs.root()).ok()?;
      let file_url = ModuleSpecifier::parse(file_name).ok()?;
      let relative_path = url.make_relative(&file_url)?;
      self.shared.source_maps.get(&relative_path)
    } else {
      self.shared.source_maps.get(file_name)
    }
    .map(Cow::Borrowed)
  }

  fn get_source_mapped_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    let specifier = ModuleSpecifier::parse(file_name).ok()?;
    let data = self
      .shared
      .modules
      .read(&specifier, VfsFileSubDataKind::Raw)
      .ok()??;

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
  ) -> Result<Cow<'a, std::path::Path>, AnyError> {
    if self.shared.modules.has_file(path) {
      // allow reading if the file is in the snapshot
      return Ok(Cow::Borrowed(path));
    }

    self
      .shared
      .npm_resolver
      .ensure_read_permission(permissions, path)
  }

  fn load_text_file_lossy(
    &self,
    path: &std::path::Path,
  ) -> Result<Cow<'static, str>, AnyError> {
    let file_entry = self.shared.vfs.file_entry(path)?;
    let file_bytes = self
      .shared
      .vfs
      .read_file_all(file_entry, VfsFileSubDataKind::ModuleGraph)?;
    Ok(from_utf8_lossy_cow(file_bytes))
  }

  fn is_maybe_cjs(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<bool, ClosestPkgJsonError> {
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

pub async fn run_standalone(data: StandaloneData) -> Result<i32, AnyError> {
  let StandaloneData {
    metadata,
    modules,
    npm_snapshot,
    root_path,
    source_maps,
    vfs,
  } = data;
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
  let root_dir_url =
    Arc::new(ModuleSpecifier::from_directory_path(&root_path).unwrap());
  let main_module = root_dir_url.join(&metadata.entrypoint_key).unwrap();
  let npm_global_cache_dir = root_path.join(".deno_compile_node_modules");
  let cache_setting = CacheSetting::Only;
  let pkg_json_resolver = Arc::new(CliPackageJsonResolver::new(sys.clone()));
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
        create_in_npm_pkg_checker(CreateInNpmPkgCheckerOptions::Managed(
          CliManagedInNpmPkgCheckerCreateOptions {
            root_cache_dir_url: npm_cache_dir.root_dir_url(),
            maybe_node_modules_path: maybe_node_modules_path.as_deref(),
          },
        ));
      let npm_resolver =
        create_cli_npm_resolver(CliNpmResolverCreateOptions::Managed(
          CliManagedNpmResolverCreateOptions {
            snapshot: CliNpmResolverManagedSnapshotOption::Specified(Some(
              snapshot,
            )),
            maybe_lockfile: None,
            http_client_provider: http_client_provider.clone(),
            npm_cache_dir,
            npm_install_deps_provider: Arc::new(
              // this is only used for installing packages, which isn't necessary with deno compile
              NpmInstallDepsProvider::empty(),
            ),
            sys: sys.clone(),
            text_only_progress_bar: progress_bar,
            cache_setting,
            maybe_node_modules_path,
            npm_system_info: Default::default(),
            npmrc,
            lifecycle_scripts: Default::default(),
          },
        ))
        .await?;
      (in_npm_pkg_checker, npm_resolver)
    }
    Some(NodeModules::Byonm {
      root_node_modules_dir,
    }) => {
      let root_node_modules_dir =
        root_node_modules_dir.map(|p| vfs.root().join(p));
      let in_npm_pkg_checker =
        create_in_npm_pkg_checker(CreateInNpmPkgCheckerOptions::Byonm);
      let npm_resolver = create_cli_npm_resolver(
        CliNpmResolverCreateOptions::Byonm(CliByonmNpmResolverCreateOptions {
          sys: sys.clone(),
          pkg_json_resolver: pkg_json_resolver.clone(),
          root_node_modules_dir,
        }),
      )
      .await?;
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
        create_in_npm_pkg_checker(CreateInNpmPkgCheckerOptions::Managed(
          CliManagedInNpmPkgCheckerCreateOptions {
            root_cache_dir_url: npm_cache_dir.root_dir_url(),
            maybe_node_modules_path: None,
          },
        ));
      let npm_resolver =
        create_cli_npm_resolver(CliNpmResolverCreateOptions::Managed(
          CliManagedNpmResolverCreateOptions {
            snapshot: CliNpmResolverManagedSnapshotOption::Specified(None),
            http_client_provider: http_client_provider.clone(),
            npm_install_deps_provider: Arc::new(
              // this is only used for installing packages, which isn't necessary with deno compile
              NpmInstallDepsProvider::empty(),
            ),
            sys: sys.clone(),
            cache_setting,
            text_only_progress_bar: progress_bar,
            npm_cache_dir,
            maybe_lockfile: None,
            maybe_node_modules_path: None,
            npm_system_info: Default::default(),
            npmrc: create_default_npmrc(),
            lifecycle_scripts: Default::default(),
          },
        ))
        .await?;
      (in_npm_pkg_checker, npm_resolver)
    }
  };

  let has_node_modules_dir = npm_resolver.root_node_modules_path().is_some();
  let node_resolver = Arc::new(NodeResolver::new(
    in_npm_pkg_checker.clone(),
    RealIsBuiltInNodeModuleChecker,
    npm_resolver.clone().into_npm_pkg_folder_resolver(),
    pkg_json_resolver.clone(),
    sys.clone(),
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
  let cache_db = Caches::new(deno_dir_provider.clone());
  let node_analysis_cache = NodeAnalysisCache::new(cache_db.node_analysis_db());
  let npm_req_resolver =
    Arc::new(CliNpmReqResolver::new(NpmReqResolverOptions {
      byonm_resolver: (npm_resolver.clone()).into_maybe_byonm(),
      sys: sys.clone(),
      in_npm_pkg_checker: in_npm_pkg_checker.clone(),
      node_resolver: node_resolver.clone(),
      npm_req_resolver: npm_resolver.clone().into_npm_req_resolver(),
    }));
  let cjs_esm_code_analyzer = CliCjsCodeAnalyzer::new(
    node_analysis_cache,
    cjs_tracker.clone(),
    fs.clone(),
    None,
  );
  let node_code_translator = Arc::new(NodeCodeTranslator::new(
    cjs_esm_code_analyzer,
    in_npm_pkg_checker,
    node_resolver.clone(),
    npm_resolver.clone().into_npm_pkg_folder_resolver(),
    pkg_json_resolver.clone(),
    sys,
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
  let code_cache = match metadata.code_cache_key {
    Some(code_cache_key) => Some(Arc::new(DenoCompileCodeCache::new(
      root_path.with_file_name(format!(
        "{}.cache",
        root_path.file_name().unwrap().to_string_lossy()
      )),
      code_cache_key,
    )) as Arc<dyn CliCodeCache>),
    None => {
      log::debug!("Code cache disabled.");
      None
    }
  };
  let module_loader_factory = StandaloneModuleLoaderFactory {
    shared: Arc::new(SharedModuleLoaderState {
      cjs_tracker: cjs_tracker.clone(),
      code_cache: code_cache.clone(),
      fs: fs.clone(),
      modules,
      node_code_translator: node_code_translator.clone(),
      node_resolver: node_resolver.clone(),
      npm_module_loader: Arc::new(NpmModuleLoader::new(
        cjs_tracker.clone(),
        fs.clone(),
        node_code_translator,
      )),
      npm_resolver: npm_resolver.clone(),
      npm_req_resolver,
      source_maps,
      vfs,
      workspace_resolver,
    }),
  };

  let permissions = {
    let mut permissions =
      metadata.permissions.to_options(/* cli_arg_urls */ &[]);
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
    code_cache,
    feature_checker,
    fs,
    None,
    None,
    None,
    Box::new(module_loader_factory),
    node_resolver,
    npm_resolver,
    pkg_json_resolver,
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
    metadata.otel_config,
    crate::args::NpmCachingStrategy::Lazy,
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

pub struct StandaloneData {
  pub metadata: Metadata,
  pub modules: StandaloneModules,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub root_path: PathBuf,
  pub source_maps: SourceMapStore,
  pub vfs: Arc<FileBackedVfs>,
}

pub enum DenoCompileModuleSource {
  String(&'static str),
  Bytes(Cow<'static, [u8]>),
}

impl DenoCompileModuleSource {
  pub fn into_for_v8(self) -> ModuleSourceCode {
    fn into_bytes(data: Cow<'static, [u8]>) -> ModuleSourceCode {
      ModuleSourceCode::Bytes(match data {
        Cow::Borrowed(d) => d.into(),
        Cow::Owned(d) => d.into_boxed_slice().into(),
      })
    }

    match self {
      // todo(https://github.com/denoland/deno_core/pull/943): store whether
      // the string is ascii or not ahead of time so we can avoid the is_ascii()
      // check in FastString::from_static
      Self::String(s) => ModuleSourceCode::String(FastString::from_static(s)),
      Self::Bytes(b) => into_bytes(b),
    }
  }
}

pub struct DenoCompileModuleData<'a> {
  pub specifier: &'a Url,
  pub media_type: MediaType,
  pub data: Cow<'static, [u8]>,
}

impl<'a> DenoCompileModuleData<'a> {
  pub fn into_parts(self) -> (&'a Url, ModuleType, DenoCompileModuleSource) {
    fn into_string_unsafe(data: Cow<'static, [u8]>) -> DenoCompileModuleSource {
      match data {
        Cow::Borrowed(d) => DenoCompileModuleSource::String(
          // SAFETY: we know this is a valid utf8 string
          unsafe { std::str::from_utf8_unchecked(d) },
        ),
        Cow::Owned(d) => DenoCompileModuleSource::Bytes(Cow::Owned(d)),
      }
    }

    let (media_type, source) = match self.media_type {
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => {
        (ModuleType::JavaScript, into_string_unsafe(self.data))
      }
      MediaType::Json => (ModuleType::Json, into_string_unsafe(self.data)),
      MediaType::Wasm => {
        (ModuleType::Wasm, DenoCompileModuleSource::Bytes(self.data))
      }
      // just assume javascript if we made it here
      MediaType::Css | MediaType::SourceMap | MediaType::Unknown => (
        ModuleType::JavaScript,
        DenoCompileModuleSource::Bytes(self.data),
      ),
    };
    (self.specifier, media_type, source)
  }
}

pub struct StandaloneModules {
  remote_modules: RemoteModulesStore,
  vfs: Arc<FileBackedVfs>,
}

impl StandaloneModules {
  pub fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a ModuleSpecifier,
  ) -> Result<Option<&'a ModuleSpecifier>, AnyError> {
    if specifier.scheme() == "file" {
      Ok(Some(specifier))
    } else {
      self.remote_modules.resolve_specifier(specifier)
    }
  }

  pub fn has_file(&self, path: &Path) -> bool {
    self.vfs.file_entry(path).is_ok()
  }

  pub fn read<'a>(
    &'a self,
    specifier: &'a ModuleSpecifier,
    kind: VfsFileSubDataKind,
  ) -> Result<Option<DenoCompileModuleData<'a>>, AnyError> {
    if specifier.scheme() == "file" {
      let path = deno_path_util::url_to_file_path(specifier)?;
      let bytes = match self.vfs.file_entry(&path) {
        Ok(entry) => self.vfs.read_file_all(entry, kind)?,
        Err(err) if err.kind() == ErrorKind::NotFound => {
          match std::fs::read(&path) {
            Ok(bytes) => Cow::Owned(bytes),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
          }
        }
        Err(err) => return Err(err.into()),
      };
      Ok(Some(DenoCompileModuleData {
        media_type: MediaType::from_specifier(specifier),
        specifier,
        data: bytes,
      }))
    } else {
      self.remote_modules.read(specifier).map(|maybe_entry| {
        maybe_entry.map(|entry| DenoCompileModuleData {
          media_type: entry.media_type,
          specifier: entry.specifier,
          data: match kind {
            VfsFileSubDataKind::Raw => entry.data,
            VfsFileSubDataKind::ModuleGraph => {
              entry.transpiled_data.unwrap_or(entry.data)
            }
          },
        })
      })
    }
  }
}
