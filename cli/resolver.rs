// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashSet;
use deno_ast::MediaType;
use deno_config::workspace::MappedResolutionDiagnostic;
use deno_config::workspace::MappedResolutionError;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;
use deno_graph::source::ResolveError;
use deno_graph::source::UnknownBuiltInNodeModuleError;
use deno_graph::NpmLoadError;
use deno_graph::NpmResolvePkgReqsResult;
use deno_npm::resolution::NpmResolutionError;
use deno_resolver::sloppy_imports::SloppyImportsCachedFs;
use deno_resolver::sloppy_imports::SloppyImportsResolver;
use deno_runtime::colors;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_runtime::deno_node::RealIsBuiltInNodeModuleChecker;
use deno_semver::package::PackageReq;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use thiserror::Error;

use crate::args::NpmCachingStrategy;
use crate::args::DENO_DISABLE_PEDANTIC_NODE_WARNINGS;
use crate::node::CliNodeCodeTranslator;
use crate::npm::CliNpmResolver;
use crate::npm::InnerCliNpmResolverRef;
use crate::sys::CliSys;
use crate::util::sync::AtomicFlag;
use crate::util::text_encoding::from_utf8_lossy_cow;

pub type CjsTracker = deno_resolver::cjs::CjsTracker<CliSys>;
pub type IsCjsResolver = deno_resolver::cjs::IsCjsResolver<CliSys>;
pub type CliSloppyImportsCachedFs = SloppyImportsCachedFs<CliSys>;
pub type CliSloppyImportsResolver =
  SloppyImportsResolver<CliSloppyImportsCachedFs>;
pub type CliDenoResolver = deno_resolver::DenoResolver<
  RealIsBuiltInNodeModuleChecker,
  CliSloppyImportsCachedFs,
  CliSys,
>;
pub type CliNpmReqResolver =
  deno_resolver::npm::NpmReqResolver<RealIsBuiltInNodeModuleChecker, CliSys>;

pub struct ModuleCodeStringSource {
  pub code: ModuleSourceCode,
  pub found_url: ModuleSpecifier,
  pub media_type: MediaType,
}

#[derive(Debug, Error, deno_error::JsError)]
#[class(type)]
#[error("{media_type} files are not supported in npm packages: {specifier}")]
pub struct NotSupportedKindInNpmError {
  pub media_type: MediaType,
  pub specifier: Url,
}

// todo(dsherret): move to module_loader.rs (it seems to be here due to use in standalone)
#[derive(Clone)]
pub struct NpmModuleLoader {
  cjs_tracker: Arc<CjsTracker>,
  fs: Arc<dyn deno_fs::FileSystem>,
  node_code_translator: Arc<CliNodeCodeTranslator>,
}

impl NpmModuleLoader {
  pub fn new(
    cjs_tracker: Arc<CjsTracker>,
    fs: Arc<dyn deno_fs::FileSystem>,
    node_code_translator: Arc<CliNodeCodeTranslator>,
  ) -> Self {
    Self {
      cjs_tracker,
      node_code_translator,
      fs,
    }
  }

  pub async fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
  ) -> Result<ModuleCodeStringSource, AnyError> {
    let file_path = specifier.to_file_path().unwrap();
    let code = self
      .fs
      .read_file_async(file_path.clone(), None)
      .await
      .map_err(AnyError::from)
      .with_context(|| {
        if file_path.is_dir() {
          // directory imports are not allowed when importing from an
          // ES module, so provide the user with a helpful error message
          let dir_path = file_path;
          let mut msg = "Directory import ".to_string();
          msg.push_str(&dir_path.to_string_lossy());
          if let Some(referrer) = &maybe_referrer {
            msg.push_str(" is not supported resolving import from ");
            msg.push_str(referrer.as_str());
            let entrypoint_name = ["index.mjs", "index.js", "index.cjs"]
              .iter()
              .find(|e| dir_path.join(e).is_file());
            if let Some(entrypoint_name) = entrypoint_name {
              msg.push_str("\nDid you mean to import ");
              msg.push_str(entrypoint_name);
              msg.push_str(" within the directory?");
            }
          }
          msg
        } else {
          let mut msg = "Unable to load ".to_string();
          msg.push_str(&file_path.to_string_lossy());
          if let Some(referrer) = &maybe_referrer {
            msg.push_str(" imported from ");
            msg.push_str(referrer.as_str());
          }
          msg
        }
      })?;

    let media_type = MediaType::from_specifier(specifier);
    if media_type.is_emittable() {
      return Err(AnyError::from(NotSupportedKindInNpmError {
        media_type,
        specifier: specifier.clone(),
      }));
    }

    let code = if self.cjs_tracker.is_maybe_cjs(specifier, media_type)? {
      // translate cjs to esm if it's cjs and inject node globals
      let code = from_utf8_lossy_cow(code);
      ModuleSourceCode::String(
        self
          .node_code_translator
          .translate_cjs_to_esm(specifier, Some(code))
          .await?
          .into_owned()
          .into(),
      )
    } else {
      // esm and json code is untouched
      ModuleSourceCode::Bytes(match code {
        Cow::Owned(bytes) => bytes.into_boxed_slice().into(),
        Cow::Borrowed(bytes) => bytes.into(),
      })
    };

    Ok(ModuleCodeStringSource {
      code,
      found_url: specifier.clone(),
      media_type: MediaType::from_specifier(specifier),
    })
  }
}

pub struct CliResolverOptions {
  pub deno_resolver: Arc<CliDenoResolver>,
  pub npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  pub bare_node_builtins_enabled: bool,
}

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct CliResolver {
  deno_resolver: Arc<CliDenoResolver>,
  npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  found_package_json_dep_flag: AtomicFlag,
  bare_node_builtins_enabled: bool,
  warned_pkgs: DashSet<PackageReq>,
}

impl CliResolver {
  pub fn new(options: CliResolverOptions) -> Self {
    Self {
      deno_resolver: options.deno_resolver,
      npm_resolver: options.npm_resolver,
      found_package_json_dep_flag: Default::default(),
      bare_node_builtins_enabled: options.bare_node_builtins_enabled,
      warned_pkgs: Default::default(),
    }
  }

  // todo(dsherret): move this off CliResolver as CliResolver is acting
  // like a factory by doing this (it's beyond its responsibility)
  pub fn create_graph_npm_resolver(
    &self,
    npm_caching: NpmCachingStrategy,
  ) -> WorkerCliNpmGraphResolver {
    WorkerCliNpmGraphResolver {
      npm_resolver: self.npm_resolver.as_ref(),
      found_package_json_dep_flag: &self.found_package_json_dep_flag,
      bare_node_builtins_enabled: self.bare_node_builtins_enabled,
      npm_caching,
    }
  }

  pub fn resolve(
    &self,
    raw_specifier: &str,
    referrer: &ModuleSpecifier,
    referrer_range_start: deno_graph::Position,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<ModuleSpecifier, ResolveError> {
    let resolution = self
      .deno_resolver
      .resolve(raw_specifier, referrer, resolution_mode, resolution_kind)
      .map_err(|err| match err.into_kind() {
        deno_resolver::DenoResolveErrorKind::MappedResolution(
          mapped_resolution_error,
        ) => match mapped_resolution_error {
          MappedResolutionError::Specifier(e) => ResolveError::Specifier(e),
          // deno_graph checks specifically for an ImportMapError
          MappedResolutionError::ImportMap(e) => ResolveError::ImportMap(e),
          MappedResolutionError::Workspace(e) => {
            ResolveError::Other(JsErrorBox::from_err(e))
          }
        },
        err => ResolveError::Other(JsErrorBox::from_err(err)),
      })?;

    if resolution.found_package_json_dep {
      // mark that we need to do an "npm install" later
      self.found_package_json_dep_flag.raise();
    }

    if let Some(diagnostic) = resolution.maybe_diagnostic {
      match &*diagnostic {
        MappedResolutionDiagnostic::ConstraintNotMatchedLocalVersion {
          reference,
          ..
        } => {
          if self.warned_pkgs.insert(reference.req().clone()) {
            log::warn!(
              "{} {}\n    at {}:{}",
              colors::yellow("Warning"),
              diagnostic,
              referrer,
              referrer_range_start,
            );
          }
        }
      }
    }

    Ok(resolution.url)
  }
}

#[derive(Debug)]
pub struct WorkerCliNpmGraphResolver<'a> {
  npm_resolver: Option<&'a Arc<dyn CliNpmResolver>>,
  found_package_json_dep_flag: &'a AtomicFlag,
  bare_node_builtins_enabled: bool,
  npm_caching: NpmCachingStrategy,
}

#[async_trait(?Send)]
impl<'a> deno_graph::source::NpmResolver for WorkerCliNpmGraphResolver<'a> {
  fn resolve_builtin_node_module(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<String>, UnknownBuiltInNodeModuleError> {
    if specifier.scheme() != "node" {
      return Ok(None);
    }

    let module_name = specifier.path().to_string();
    if is_builtin_node_module(&module_name) {
      Ok(Some(module_name))
    } else {
      Err(UnknownBuiltInNodeModuleError { module_name })
    }
  }

  fn on_resolve_bare_builtin_node_module(
    &self,
    module_name: &str,
    range: &deno_graph::Range,
  ) {
    let start = range.range.start;
    let specifier = &range.specifier;
    if !*DENO_DISABLE_PEDANTIC_NODE_WARNINGS {
      log::warn!("{} Resolving \"{module_name}\" as \"node:{module_name}\" at {specifier}:{start}. If you want to use a built-in Node module, add a \"node:\" prefix.", colors::yellow("Warning"))
    }
  }

  fn load_and_cache_npm_package_info(&self, package_name: &str) {
    match self.npm_resolver {
      Some(npm_resolver) if npm_resolver.as_managed().is_some() => {
        let npm_resolver = npm_resolver.clone();
        let package_name = package_name.to_string();
        deno_core::unsync::spawn(async move {
          if let Some(managed) = npm_resolver.as_managed() {
            let _ignore = managed.cache_package_info(&package_name).await;
          }
        });
      }
      _ => {}
    }
  }

  async fn resolve_pkg_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> NpmResolvePkgReqsResult {
    match &self.npm_resolver {
      Some(npm_resolver) => {
        let npm_resolver = match npm_resolver.as_inner() {
          InnerCliNpmResolverRef::Managed(npm_resolver) => npm_resolver,
          // if we are using byonm, then this should never be called because
          // we don't use deno_graph's npm resolution in this case
          InnerCliNpmResolverRef::Byonm(_) => unreachable!(),
        };

        let top_level_result = if self.found_package_json_dep_flag.is_raised() {
          npm_resolver
            .ensure_top_level_package_json_install()
            .await
            .map(|_| ())
        } else {
          Ok(())
        };

        let result = npm_resolver
          .add_package_reqs_raw(
            package_reqs,
            match self.npm_caching {
              NpmCachingStrategy::Eager => {
                Some(crate::npm::PackageCaching::All)
              }
              NpmCachingStrategy::Lazy => {
                Some(crate::npm::PackageCaching::Only(package_reqs.into()))
              }
              NpmCachingStrategy::Manual => None,
            },
          )
          .await;

        NpmResolvePkgReqsResult {
          results: result
            .results
            .into_iter()
            .map(|r| {
              r.map_err(|err| match err {
                NpmResolutionError::Registry(e) => {
                  NpmLoadError::RegistryInfo(Arc::new(e))
                }
                NpmResolutionError::Resolution(e) => {
                  NpmLoadError::PackageReqResolution(Arc::new(e))
                }
                NpmResolutionError::DependencyEntry(e) => {
                  NpmLoadError::PackageReqResolution(Arc::new(e))
                }
              })
            })
            .collect(),
          dep_graph_result: match top_level_result {
            Ok(()) => result
              .dependencies_result
              .map_err(|e| Arc::new(e) as Arc<dyn deno_error::JsErrorClass>),
            Err(err) => Err(Arc::new(err)),
          },
        }
      }
      None => {
        let err = Arc::new(JsErrorBox::generic(
          "npm specifiers were requested; but --no-npm is specified",
        ));
        NpmResolvePkgReqsResult {
          results: package_reqs
            .iter()
            .map(|_| Err(NpmLoadError::RegistryInfo(err.clone())))
            .collect(),
          dep_graph_result: Err(err),
        }
      }
    }
  }

  fn enables_bare_builtin_node_module(&self) -> bool {
    self.bare_node_builtins_enabled
  }
}
