// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashSet;
use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;
use deno_graph::source::ResolveError;
use deno_graph::source::UnknownBuiltInNodeModuleError;
use deno_graph::NpmLoadError;
use deno_graph::NpmResolvePkgReqsResult;
use deno_npm::resolution::NpmResolutionError;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::workspace::MappedResolutionDiagnostic;
use deno_resolver::workspace::MappedResolutionError;
use deno_runtime::colors;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_semver::package::PackageReq;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;

use crate::args::NpmCachingStrategy;
use crate::args::DENO_DISABLE_PEDANTIC_NODE_WARNINGS;
use crate::npm::installer::NpmInstaller;
use crate::npm::installer::PackageCaching;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;
use crate::util::sync::AtomicFlag;

pub type CliCjsTracker =
  deno_resolver::cjs::CjsTracker<DenoInNpmPackageChecker, CliSys>;
pub type CliIsCjsResolver =
  deno_resolver::cjs::IsCjsResolver<DenoInNpmPackageChecker, CliSys>;
pub type CliDenoResolver = deno_resolver::DenoResolver<
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliNpmReqResolver = deno_resolver::npm::NpmReqResolver<
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;

#[derive(Debug, Default)]
pub struct FoundPackageJsonDepFlag(AtomicFlag);

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct CliResolver {
  deno_resolver: Arc<CliDenoResolver>,
  found_package_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  warned_pkgs: DashSet<PackageReq>,
}

impl CliResolver {
  pub fn new(
    deno_resolver: Arc<CliDenoResolver>,
    found_package_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  ) -> Self {
    Self {
      deno_resolver,
      found_package_json_dep_flag,
      warned_pkgs: Default::default(),
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
      self.found_package_json_dep_flag.0.raise();
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
pub struct CliNpmGraphResolver {
  npm_installer: Option<Arc<NpmInstaller>>,
  found_package_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  bare_node_builtins_enabled: bool,
  npm_caching: NpmCachingStrategy,
}

impl CliNpmGraphResolver {
  pub fn new(
    npm_installer: Option<Arc<NpmInstaller>>,
    found_package_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
    bare_node_builtins_enabled: bool,
    npm_caching: NpmCachingStrategy,
  ) -> Self {
    Self {
      npm_installer,
      found_package_json_dep_flag,
      bare_node_builtins_enabled,
      npm_caching,
    }
  }
}

#[async_trait(?Send)]
impl deno_graph::source::NpmResolver for CliNpmGraphResolver {
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
    if let Some(npm_installer) = &self.npm_installer {
      let npm_installer = npm_installer.clone();
      let package_name = package_name.to_string();
      deno_core::unsync::spawn(async move {
        let _ignore = npm_installer.cache_package_info(&package_name).await;
      });
    }
  }

  async fn resolve_pkg_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> NpmResolvePkgReqsResult {
    match &self.npm_installer {
      Some(npm_installer) => {
        let top_level_result = if self.found_package_json_dep_flag.0.is_raised()
        {
          npm_installer
            .ensure_top_level_package_json_install()
            .await
            .map(|_| ())
        } else {
          Ok(())
        };

        let result = npm_installer
          .add_package_reqs_raw(
            package_reqs,
            match self.npm_caching {
              NpmCachingStrategy::Eager => Some(PackageCaching::All),
              NpmCachingStrategy::Lazy => {
                Some(PackageCaching::Only(package_reqs.into()))
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
