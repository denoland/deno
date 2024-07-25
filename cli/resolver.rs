// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use async_trait::async_trait;
use dashmap::DashMap;
use dashmap::DashSet;
use deno_ast::MediaType;
use deno_config::workspace::MappedResolution;
use deno_config::workspace::MappedResolutionError;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolutionMode;
use deno_graph::source::ResolveError;
use deno_graph::source::Resolver;
use deno_graph::source::UnknownBuiltInNodeModuleError;
use deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE;
use deno_graph::NpmLoadError;
use deno_graph::NpmResolvePkgReqsResult;
use deno_npm::resolution::NpmResolutionError;
use deno_package_json::PackageJsonDepValue;
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::errors::ClosestPkgJsonError;
use deno_runtime::deno_node::errors::NodeResolveError;
use deno_runtime::deno_node::errors::NodeResolveErrorKind;
use deno_runtime::deno_node::errors::PackageFolderResolveErrorKind;
use deno_runtime::deno_node::errors::PackageFolderResolveIoError;
use deno_runtime::deno_node::errors::PackageNotFoundError;
use deno_runtime::deno_node::errors::PackageResolveErrorKind;
use deno_runtime::deno_node::errors::UrlToNodeResolutionError;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_runtime::deno_node::NodeModuleKind;
use deno_runtime::deno_node::NodeResolution;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::fs_util::specifier_to_file_path;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::args::JsxImportSourceConfig;
use crate::args::DENO_DISABLE_PEDANTIC_NODE_WARNINGS;
use crate::node::CliNodeCodeTranslator;
use crate::npm::CliNpmResolver;
use crate::npm::InnerCliNpmResolverRef;
use crate::util::sync::AtomicFlag;

pub struct ModuleCodeStringSource {
  pub code: ModuleSourceCode,
  pub found_url: ModuleSpecifier,
  pub media_type: MediaType,
}

#[derive(Debug)]
pub struct CliNodeResolver {
  cjs_resolutions: Arc<CjsResolutionStore>,
  fs: Arc<dyn deno_fs::FileSystem>,
  node_resolver: Arc<NodeResolver>,
  // todo(dsherret): remove this pub(crate)
  pub(crate) npm_resolver: Arc<dyn CliNpmResolver>,
}

impl CliNodeResolver {
  pub fn new(
    cjs_resolutions: Arc<CjsResolutionStore>,
    fs: Arc<dyn deno_fs::FileSystem>,
    node_resolver: Arc<NodeResolver>,
    npm_resolver: Arc<dyn CliNpmResolver>,
  ) -> Self {
    Self {
      cjs_resolutions,
      fs,
      node_resolver,
      npm_resolver,
    }
  }

  pub fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    self.npm_resolver.in_npm_package(specifier)
  }

  pub fn get_closest_package_json(
    &self,
    referrer: &ModuleSpecifier,
  ) -> Result<Option<Arc<PackageJson>>, ClosestPkgJsonError> {
    self.node_resolver.get_closest_package_json(referrer)
  }

  pub fn resolve_if_for_npm_pkg(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<Option<NodeResolution>, AnyError> {
    let resolution_result = self.resolve(specifier, referrer, mode);
    match resolution_result {
      Ok(res) => Ok(Some(res)),
      Err(err) => {
        let err = err.into_kind();
        match err {
          NodeResolveErrorKind::RelativeJoin(_)
          | NodeResolveErrorKind::PackageImportsResolve(_)
          | NodeResolveErrorKind::UnsupportedEsmUrlScheme(_)
          | NodeResolveErrorKind::DataUrlReferrer(_)
          | NodeResolveErrorKind::TypesNotFound(_)
          | NodeResolveErrorKind::FinalizeResolution(_)
          | NodeResolveErrorKind::UrlToNodeResolution(_) => Err(err.into()),
          NodeResolveErrorKind::PackageResolve(err) => {
            let err = err.into_kind();
            match err {
              PackageResolveErrorKind::ClosestPkgJson(_)
              | PackageResolveErrorKind::InvalidModuleSpecifier(_)
              | PackageResolveErrorKind::ExportsResolve(_)
              | PackageResolveErrorKind::SubpathResolve(_) => Err(err.into()),
              PackageResolveErrorKind::PackageFolderResolve(err) => {
                match err.as_kind() {
                  PackageFolderResolveErrorKind::Io(
                    PackageFolderResolveIoError { package_name, .. },
                  )
                  | PackageFolderResolveErrorKind::PackageNotFound(
                    PackageNotFoundError { package_name, .. },
                  ) => {
                    if self.in_npm_package(referrer) {
                      return Err(err.into());
                    }
                    if let Some(byonm_npm_resolver) =
                      self.npm_resolver.as_byonm()
                    {
                      if byonm_npm_resolver
                        .find_ancestor_package_json_with_dep(
                          package_name,
                          referrer,
                        )
                        .is_some()
                      {
                        return Err(anyhow!(
                        concat!(
                        "Could not resolve \"{}\", but found it in a package.json. ",
                        "Deno expects the node_modules/ directory to be up to date. ",
                        "Did you forget to run `npm install`?"
                      ),
                        specifier
                      ));
                      }
                    }
                    Ok(None)
                  }
                  PackageFolderResolveErrorKind::ReferrerNotFound(_) => {
                    if self.in_npm_package(referrer) {
                      return Err(err.into());
                    }
                    Ok(None)
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<NodeResolution, NodeResolveError> {
    let referrer_kind = if self.cjs_resolutions.contains(referrer) {
      NodeModuleKind::Cjs
    } else {
      NodeModuleKind::Esm
    };

    let res =
      self
        .node_resolver
        .resolve(specifier, referrer, referrer_kind, mode)?;
    Ok(self.handle_node_resolution(res))
  }

  pub fn resolve_req_reference(
    &self,
    req_ref: &NpmPackageReqReference,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<NodeResolution, AnyError> {
    self.resolve_req_with_sub_path(
      req_ref.req(),
      req_ref.sub_path(),
      referrer,
      mode,
    )
  }

  pub fn resolve_req_with_sub_path(
    &self,
    req: &PackageReq,
    sub_path: Option<&str>,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<NodeResolution, AnyError> {
    let package_folder = self
      .npm_resolver
      .resolve_pkg_folder_from_deno_module_req(req, referrer)?;
    let resolution_result = self.resolve_package_sub_path_from_deno_module(
      &package_folder,
      sub_path,
      Some(referrer),
      mode,
    );
    match resolution_result {
      Ok(resolution) => Ok(resolution),
      Err(err) => {
        if self.npm_resolver.as_byonm().is_some() {
          let package_json_path = package_folder.join("package.json");
          if !self.fs.exists_sync(&package_json_path) {
            return Err(anyhow!(
              "Could not find '{}'. Deno expects the node_modules/ directory to be up to date. Did you forget to run `{}`?",
              package_json_path.display(),
              if *crate::args::DENO_FUTURE {
                "deno install"
              } else {
                "npm install"
              },
            ));
          }
        }
        Err(err)
      }
    }
  }

  pub fn resolve_package_sub_path_from_deno_module(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
    maybe_referrer: Option<&ModuleSpecifier>,
    mode: NodeResolutionMode,
  ) -> Result<NodeResolution, AnyError> {
    let res = self
      .node_resolver
      .resolve_package_subpath_from_deno_module(
        package_folder,
        sub_path,
        maybe_referrer,
        mode,
      )?;
    Ok(self.handle_node_resolution(res))
  }

  pub fn handle_if_in_node_modules(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<ModuleSpecifier>, AnyError> {
    // skip canonicalizing if we definitely know it's unnecessary
    if specifier.scheme() == "file"
      && specifier.path().contains("/node_modules/")
    {
      // Specifiers in the node_modules directory are canonicalized
      // so canoncalize then check if it's in the node_modules directory.
      // If so, check if we need to store this specifier as being a CJS
      // resolution.
      let specifier =
        crate::node::resolve_specifier_into_node_modules(specifier);
      if self.in_npm_package(&specifier) {
        let resolution =
          self.node_resolver.url_to_node_resolution(specifier)?;
        if let NodeResolution::CommonJs(specifier) = &resolution {
          self.cjs_resolutions.insert(specifier.clone());
        }
        return Ok(Some(resolution.into_url()));
      }
    }

    Ok(None)
  }

  pub fn url_to_node_resolution(
    &self,
    specifier: ModuleSpecifier,
  ) -> Result<NodeResolution, UrlToNodeResolutionError> {
    self.node_resolver.url_to_node_resolution(specifier)
  }

  fn handle_node_resolution(
    &self,
    resolution: NodeResolution,
  ) -> NodeResolution {
    if let NodeResolution::CommonJs(specifier) = &resolution {
      // remember that this was a common js resolution
      self.cjs_resolutions.insert(specifier.clone());
    }
    resolution
  }
}

#[derive(Clone)]
pub struct NpmModuleLoader {
  cjs_resolutions: Arc<CjsResolutionStore>,
  node_code_translator: Arc<CliNodeCodeTranslator>,
  fs: Arc<dyn deno_fs::FileSystem>,
  node_resolver: Arc<CliNodeResolver>,
}

impl NpmModuleLoader {
  pub fn new(
    cjs_resolutions: Arc<CjsResolutionStore>,
    node_code_translator: Arc<CliNodeCodeTranslator>,
    fs: Arc<dyn deno_fs::FileSystem>,
    node_resolver: Arc<CliNodeResolver>,
  ) -> Self {
    Self {
      cjs_resolutions,
      node_code_translator,
      fs,
      node_resolver,
    }
  }

  pub async fn load_if_in_npm_package(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
  ) -> Option<Result<ModuleCodeStringSource, AnyError>> {
    if self.node_resolver.in_npm_package(specifier) {
      Some(self.load(specifier, maybe_referrer).await)
    } else {
      None
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

    let code = if self.cjs_resolutions.contains(specifier) {
      // translate cjs to esm if it's cjs and inject node globals
      let code = match String::from_utf8_lossy(&code) {
        Cow::Owned(code) => code,
        // SAFETY: `String::from_utf8_lossy` guarantees that the result is valid
        // UTF-8 if `Cow::Borrowed` is returned.
        Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(code) },
      };
      ModuleSourceCode::String(
        self
          .node_code_translator
          .translate_cjs_to_esm(specifier, Some(code))
          .await?
          .into(),
      )
    } else {
      // esm and json code is untouched
      ModuleSourceCode::Bytes(code.into_boxed_slice().into())
    };
    Ok(ModuleCodeStringSource {
      code,
      found_url: specifier.clone(),
      media_type: MediaType::from_specifier(specifier),
    })
  }
}

/// Keeps track of what module specifiers were resolved as CJS.
#[derive(Debug, Default)]
pub struct CjsResolutionStore(DashSet<ModuleSpecifier>);

impl CjsResolutionStore {
  pub fn contains(&self, specifier: &ModuleSpecifier) -> bool {
    self.0.contains(specifier)
  }

  pub fn insert(&self, specifier: ModuleSpecifier) {
    self.0.insert(specifier);
  }
}

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct CliGraphResolver {
  node_resolver: Option<Arc<CliNodeResolver>>,
  npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  sloppy_imports_resolver: Option<Arc<SloppyImportsResolver>>,
  workspace_resolver: Arc<WorkspaceResolver>,
  maybe_default_jsx_import_source: Option<String>,
  maybe_default_jsx_import_source_types: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
  maybe_vendor_specifier: Option<ModuleSpecifier>,
  found_package_json_dep_flag: AtomicFlag,
  bare_node_builtins_enabled: bool,
}

pub struct CliGraphResolverOptions<'a> {
  pub node_resolver: Option<Arc<CliNodeResolver>>,
  pub npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  pub sloppy_imports_resolver: Option<Arc<SloppyImportsResolver>>,
  pub workspace_resolver: Arc<WorkspaceResolver>,
  pub bare_node_builtins_enabled: bool,
  pub maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
  pub maybe_vendor_dir: Option<&'a PathBuf>,
}

impl CliGraphResolver {
  pub fn new(options: CliGraphResolverOptions) -> Self {
    Self {
      node_resolver: options.node_resolver,
      npm_resolver: options.npm_resolver,
      sloppy_imports_resolver: options.sloppy_imports_resolver,
      workspace_resolver: options.workspace_resolver,
      maybe_default_jsx_import_source: options
        .maybe_jsx_import_source_config
        .as_ref()
        .and_then(|c| c.default_specifier.clone()),
      maybe_default_jsx_import_source_types: options
        .maybe_jsx_import_source_config
        .as_ref()
        .and_then(|c| c.default_types_specifier.clone()),
      maybe_jsx_import_source_module: options
        .maybe_jsx_import_source_config
        .map(|c| c.module),
      maybe_vendor_specifier: options
        .maybe_vendor_dir
        .and_then(|v| ModuleSpecifier::from_directory_path(v).ok()),
      found_package_json_dep_flag: Default::default(),
      bare_node_builtins_enabled: options.bare_node_builtins_enabled,
    }
  }

  pub fn as_graph_resolver(&self) -> &dyn Resolver {
    self
  }

  pub fn create_graph_npm_resolver(&self) -> WorkerCliNpmGraphResolver {
    WorkerCliNpmGraphResolver {
      npm_resolver: self.npm_resolver.as_ref(),
      found_package_json_dep_flag: &self.found_package_json_dep_flag,
      bare_node_builtins_enabled: self.bare_node_builtins_enabled,
    }
  }
}

impl Resolver for CliGraphResolver {
  fn default_jsx_import_source(&self) -> Option<String> {
    self.maybe_default_jsx_import_source.clone()
  }

  fn default_jsx_import_source_types(&self) -> Option<String> {
    self.maybe_default_jsx_import_source_types.clone()
  }

  fn jsx_import_source_module(&self) -> &str {
    self
      .maybe_jsx_import_source_module
      .as_deref()
      .unwrap_or(DEFAULT_JSX_IMPORT_SOURCE_MODULE)
  }

  fn resolve(
    &self,
    specifier: &str,
    referrer_range: &deno_graph::Range,
    mode: ResolutionMode,
  ) -> Result<ModuleSpecifier, ResolveError> {
    fn to_node_mode(mode: ResolutionMode) -> NodeResolutionMode {
      match mode {
        ResolutionMode::Execution => NodeResolutionMode::Execution,
        ResolutionMode::Types => NodeResolutionMode::Types,
      }
    }

    let referrer = &referrer_range.specifier;

    // Use node resolution if we're in an npm package
    if let Some(node_resolver) = self.node_resolver.as_ref() {
      if referrer.scheme() == "file" && node_resolver.in_npm_package(referrer) {
        return node_resolver
          .resolve(specifier, referrer, to_node_mode(mode))
          .map(|res| res.into_url())
          .map_err(|e| ResolveError::Other(e.into()));
      }
    }

    // Attempt to resolve with the workspace resolver
    let result: Result<_, ResolveError> = self
      .workspace_resolver
      .resolve(specifier, referrer)
      .map_err(|err| match err {
        MappedResolutionError::Specifier(err) => ResolveError::Specifier(err),
        MappedResolutionError::ImportMap(err) => {
          ResolveError::Other(err.into())
        }
      });
    let result = match result {
      Ok(resolution) => match resolution {
        MappedResolution::Normal(specifier)
        | MappedResolution::ImportMap(specifier) => {
          // do sloppy imports resolution if enabled
          if let Some(sloppy_imports_resolver) = &self.sloppy_imports_resolver {
            Ok(
              sloppy_imports_resolver
                .resolve(&specifier, mode)
                .map(|s| s.into_specifier())
                .unwrap_or(specifier),
            )
          } else {
            Ok(specifier)
          }
        }
        MappedResolution::WorkspaceNpmPackage {
          target_pkg_json: pkg_json,
          sub_path,
          ..
        } => self
          .node_resolver
          .as_ref()
          .unwrap()
          .resolve_package_sub_path_from_deno_module(
            pkg_json.dir_path(),
            sub_path.as_deref(),
            Some(referrer),
            to_node_mode(mode),
          )
          .map_err(ResolveError::Other)
          .map(|res| res.into_url()),
        MappedResolution::PackageJson {
          dep_result,
          alias,
          sub_path,
          ..
        } => {
          // found a specifier in the package.json, so mark that
          // we need to do an "npm install" later
          self.found_package_json_dep_flag.raise();

          dep_result
            .as_ref()
            .map_err(|e| ResolveError::Other(e.clone().into()))
            .and_then(|dep| match dep {
              PackageJsonDepValue::Req(req) => {
                ModuleSpecifier::parse(&format!(
                  "npm:{}{}",
                  req,
                  sub_path.map(|s| format!("/{}", s)).unwrap_or_default()
                ))
                .map_err(|e| ResolveError::Other(e.into()))
              }
              PackageJsonDepValue::Workspace(version_req) => self
                .workspace_resolver
                .resolve_workspace_pkg_json_folder_for_pkg_json_dep(
                  alias,
                  version_req,
                )
                .map_err(|e| ResolveError::Other(e.into()))
                .and_then(|pkg_folder| {
                  Ok(
                    self
                      .node_resolver
                      .as_ref()
                      .unwrap()
                      .resolve_package_sub_path_from_deno_module(
                        pkg_folder,
                        sub_path.as_deref(),
                        Some(referrer),
                        to_node_mode(mode),
                      )?
                      .into_url(),
                  )
                }),
            })
        }
      },
      Err(err) => Err(err),
    };

    // When the user is vendoring, don't allow them to import directly from the vendor/ directory
    // as it might cause them confusion or duplicate dependencies. Additionally, this folder has
    // special treatment in the language server so it will definitely cause issues/confusion there
    // if they do this.
    if let Some(vendor_specifier) = &self.maybe_vendor_specifier {
      if let Ok(specifier) = &result {
        if specifier.as_str().starts_with(vendor_specifier.as_str()) {
          return Err(ResolveError::Other(anyhow!("Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring.")));
        }
      }
    }

    let Some(node_resolver) = &self.node_resolver else {
      return result;
    };

    let is_byonm = self
      .npm_resolver
      .as_ref()
      .is_some_and(|r| r.as_byonm().is_some());
    match result {
      Ok(specifier) => {
        if let Ok(npm_req_ref) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          // check if the npm specifier resolves to a workspace member
          if let Some(pkg_folder) = self
            .workspace_resolver
            .resolve_workspace_pkg_json_folder_for_npm_specifier(
              npm_req_ref.req(),
            )
          {
            return Ok(
              node_resolver
                .resolve_package_sub_path_from_deno_module(
                  pkg_folder,
                  npm_req_ref.sub_path(),
                  Some(referrer),
                  to_node_mode(mode),
                )?
                .into_url(),
            );
          }

          // do npm resolution for byonm
          if is_byonm {
            return node_resolver
              .resolve_req_reference(&npm_req_ref, referrer, to_node_mode(mode))
              .map(|res| res.into_url())
              .map_err(|err| err.into());
          }
        }

        Ok(match node_resolver.handle_if_in_node_modules(&specifier)? {
          Some(specifier) => specifier,
          None => specifier,
        })
      }
      Err(err) => {
        // If byonm, check if the bare specifier resolves to an npm package
        if is_byonm && referrer.scheme() == "file" {
          let maybe_resolution = node_resolver
            .resolve_if_for_npm_pkg(specifier, referrer, to_node_mode(mode))
            .map_err(ResolveError::Other)?;
          if let Some(res) = maybe_resolution {
            return Ok(res.into_url());
          }
        }

        Err(err)
      }
    }
  }
}

#[derive(Debug)]
pub struct WorkerCliNpmGraphResolver<'a> {
  npm_resolver: Option<&'a Arc<dyn CliNpmResolver>>,
  found_package_json_dep_flag: &'a AtomicFlag,
  bare_node_builtins_enabled: bool,
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
    let deno_graph::Range {
      start, specifier, ..
    } = range;
    let line = start.line + 1;
    let column = start.character + 1;
    if !*DENO_DISABLE_PEDANTIC_NODE_WARNINGS {
      log::warn!("Warning: Resolving \"{module_name}\" as \"node:{module_name}\" at {specifier}:{line}:{column}. If you want to use a built-in Node module, add a \"node:\" prefix.")
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

        let result = npm_resolver.add_package_reqs_raw(package_reqs).await;

        NpmResolvePkgReqsResult {
          results: result
            .results
            .into_iter()
            .map(|r| {
              r.map_err(|err| match err {
                NpmResolutionError::Registry(e) => {
                  NpmLoadError::RegistryInfo(Arc::new(e.into()))
                }
                NpmResolutionError::Resolution(e) => {
                  NpmLoadError::PackageReqResolution(Arc::new(e.into()))
                }
                NpmResolutionError::DependencyEntry(e) => {
                  NpmLoadError::PackageReqResolution(Arc::new(e.into()))
                }
              })
            })
            .collect(),
          dep_graph_result: match top_level_result {
            Ok(()) => result.dependencies_result.map_err(Arc::new),
            Err(err) => Err(Arc::new(err)),
          },
        }
      }
      None => {
        let err = Arc::new(anyhow!(
          "npm specifiers were requested; but --no-npm is specified"
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SloppyImportsFsEntry {
  File,
  Dir,
}

impl SloppyImportsFsEntry {
  pub fn from_fs_stat(
    stat: &deno_runtime::deno_io::fs::FsStat,
  ) -> Option<SloppyImportsFsEntry> {
    if stat.is_file {
      Some(SloppyImportsFsEntry::File)
    } else if stat.is_directory {
      Some(SloppyImportsFsEntry::Dir)
    } else {
      None
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SloppyImportsResolution {
  /// Ex. `./file.js` to `./file.ts`
  JsToTs(ModuleSpecifier),
  /// Ex. `./file` to `./file.ts`
  NoExtension(ModuleSpecifier),
  /// Ex. `./dir` to `./dir/index.ts`
  Directory(ModuleSpecifier),
}

impl SloppyImportsResolution {
  pub fn as_specifier(&self) -> &ModuleSpecifier {
    match self {
      Self::JsToTs(specifier) => specifier,
      Self::NoExtension(specifier) => specifier,
      Self::Directory(specifier) => specifier,
    }
  }

  pub fn into_specifier(self) -> ModuleSpecifier {
    match self {
      Self::JsToTs(specifier) => specifier,
      Self::NoExtension(specifier) => specifier,
      Self::Directory(specifier) => specifier,
    }
  }

  pub fn as_suggestion_message(&self) -> String {
    format!("Maybe {}", self.as_base_message())
  }

  pub fn as_quick_fix_message(&self) -> String {
    let message = self.as_base_message();
    let mut chars = message.chars();
    format!(
      "{}{}.",
      chars.next().unwrap().to_uppercase(),
      chars.as_str()
    )
  }

  fn as_base_message(&self) -> String {
    match self {
      SloppyImportsResolution::JsToTs(specifier) => {
        let media_type = MediaType::from_specifier(specifier);
        format!("change the extension to '{}'", media_type.as_ts_extension())
      }
      SloppyImportsResolution::NoExtension(specifier) => {
        let media_type = MediaType::from_specifier(specifier);
        format!("add a '{}' extension", media_type.as_ts_extension())
      }
      SloppyImportsResolution::Directory(specifier) => {
        let file_name = specifier
          .path()
          .rsplit_once('/')
          .map(|(_, file_name)| file_name)
          .unwrap_or(specifier.path());
        format!("specify path to '{}' file in directory instead", file_name)
      }
    }
  }
}

#[derive(Debug)]
pub struct SloppyImportsResolver {
  fs: Arc<dyn FileSystem>,
  cache: Option<DashMap<PathBuf, Option<SloppyImportsFsEntry>>>,
}

impl SloppyImportsResolver {
  pub fn new(fs: Arc<dyn FileSystem>) -> Self {
    Self {
      fs,
      cache: Some(Default::default()),
    }
  }

  pub fn new_without_stat_cache(fs: Arc<dyn FileSystem>) -> Self {
    Self { fs, cache: None }
  }

  pub fn resolve(
    &self,
    specifier: &ModuleSpecifier,
    mode: ResolutionMode,
  ) -> Option<SloppyImportsResolution> {
    fn path_without_ext(
      path: &Path,
      media_type: MediaType,
    ) -> Option<Cow<str>> {
      let old_path_str = path.to_string_lossy();
      match media_type {
        MediaType::Unknown => Some(old_path_str),
        _ => old_path_str
          .strip_suffix(media_type.as_ts_extension())
          .map(|s| Cow::Owned(s.to_string())),
      }
    }

    fn media_types_to_paths(
      path_no_ext: &str,
      original_media_type: MediaType,
      probe_media_type_types: Vec<MediaType>,
      reason: SloppyImportsResolutionReason,
    ) -> Vec<(PathBuf, SloppyImportsResolutionReason)> {
      probe_media_type_types
        .into_iter()
        .filter(|media_type| *media_type != original_media_type)
        .map(|media_type| {
          (
            PathBuf::from(format!(
              "{}{}",
              path_no_ext,
              media_type.as_ts_extension()
            )),
            reason,
          )
        })
        .collect::<Vec<_>>()
    }

    if specifier.scheme() != "file" {
      return None;
    }

    let path = specifier_to_file_path(specifier).ok()?;

    #[derive(Clone, Copy)]
    enum SloppyImportsResolutionReason {
      JsToTs,
      NoExtension,
      Directory,
    }

    let probe_paths: Vec<(PathBuf, SloppyImportsResolutionReason)> =
      match self.stat_sync(&path) {
        Some(SloppyImportsFsEntry::File) => {
          if mode.is_types() {
            let media_type = MediaType::from_specifier(specifier);
            // attempt to resolve the .d.ts file before the .js file
            let probe_media_type_types = match media_type {
              MediaType::JavaScript => {
                vec![(MediaType::Dts), MediaType::JavaScript]
              }
              MediaType::Mjs => {
                vec![MediaType::Dmts, MediaType::Dts, MediaType::Mjs]
              }
              MediaType::Cjs => {
                vec![MediaType::Dcts, MediaType::Dts, MediaType::Cjs]
              }
              _ => return None,
            };
            let path_no_ext = path_without_ext(&path, media_type)?;
            media_types_to_paths(
              &path_no_ext,
              media_type,
              probe_media_type_types,
              SloppyImportsResolutionReason::JsToTs,
            )
          } else {
            return None;
          }
        }
        entry @ None | entry @ Some(SloppyImportsFsEntry::Dir) => {
          let media_type = MediaType::from_specifier(specifier);
          let probe_media_type_types = match media_type {
            MediaType::JavaScript => (
              if mode.is_types() {
                vec![MediaType::TypeScript, MediaType::Tsx, MediaType::Dts]
              } else {
                vec![MediaType::TypeScript, MediaType::Tsx]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::Jsx => {
              (vec![MediaType::Tsx], SloppyImportsResolutionReason::JsToTs)
            }
            MediaType::Mjs => (
              if mode.is_types() {
                vec![MediaType::Mts, MediaType::Dmts, MediaType::Dts]
              } else {
                vec![MediaType::Mts]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::Cjs => (
              if mode.is_types() {
                vec![MediaType::Cts, MediaType::Dcts, MediaType::Dts]
              } else {
                vec![MediaType::Cts]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Dts
            | MediaType::Dmts
            | MediaType::Dcts
            | MediaType::Tsx
            | MediaType::Json
            | MediaType::Wasm
            | MediaType::TsBuildInfo
            | MediaType::SourceMap => {
              return None;
            }
            MediaType::Unknown => (
              if mode.is_types() {
                vec![
                  MediaType::TypeScript,
                  MediaType::Tsx,
                  MediaType::Mts,
                  MediaType::Dts,
                  MediaType::Dmts,
                  MediaType::Dcts,
                  MediaType::JavaScript,
                  MediaType::Jsx,
                  MediaType::Mjs,
                ]
              } else {
                vec![
                  MediaType::TypeScript,
                  MediaType::JavaScript,
                  MediaType::Tsx,
                  MediaType::Jsx,
                  MediaType::Mts,
                  MediaType::Mjs,
                ]
              },
              SloppyImportsResolutionReason::NoExtension,
            ),
          };
          let mut probe_paths = match path_without_ext(&path, media_type) {
            Some(path_no_ext) => media_types_to_paths(
              &path_no_ext,
              media_type,
              probe_media_type_types.0,
              probe_media_type_types.1,
            ),
            None => vec![],
          };

          if matches!(entry, Some(SloppyImportsFsEntry::Dir)) {
            // try to resolve at the index file
            if mode.is_types() {
              probe_paths.push((
                path.join("index.ts"),
                SloppyImportsResolutionReason::Directory,
              ));

              probe_paths.push((
                path.join("index.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.d.ts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.d.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.js"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mjs"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.tsx"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.jsx"),
                SloppyImportsResolutionReason::Directory,
              ));
            } else {
              probe_paths.push((
                path.join("index.ts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.tsx"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.js"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mjs"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.jsx"),
                SloppyImportsResolutionReason::Directory,
              ));
            }
          }
          if probe_paths.is_empty() {
            return None;
          }
          probe_paths
        }
      };

    for (probe_path, reason) in probe_paths {
      if self.stat_sync(&probe_path) == Some(SloppyImportsFsEntry::File) {
        if let Ok(specifier) = ModuleSpecifier::from_file_path(probe_path) {
          match reason {
            SloppyImportsResolutionReason::JsToTs => {
              return Some(SloppyImportsResolution::JsToTs(specifier));
            }
            SloppyImportsResolutionReason::NoExtension => {
              return Some(SloppyImportsResolution::NoExtension(specifier));
            }
            SloppyImportsResolutionReason::Directory => {
              return Some(SloppyImportsResolution::Directory(specifier));
            }
          }
        }
      }
    }

    None
  }

  fn stat_sync(&self, path: &Path) -> Option<SloppyImportsFsEntry> {
    if let Some(cache) = &self.cache {
      if let Some(entry) = cache.get(path) {
        return *entry;
      }
    }

    let entry = self
      .fs
      .stat_sync(path)
      .ok()
      .and_then(|stat| SloppyImportsFsEntry::from_fs_stat(&stat));
    if let Some(cache) = &self.cache {
      cache.insert(path.to_owned(), entry);
    }
    entry
  }
}

#[cfg(test)]
mod test {
  use test_util::TestContext;

  use super::*;

  #[test]
  fn test_unstable_sloppy_imports() {
    fn resolve(specifier: &ModuleSpecifier) -> Option<SloppyImportsResolution> {
      resolve_with_mode(specifier, ResolutionMode::Execution)
    }

    fn resolve_types(
      specifier: &ModuleSpecifier,
    ) -> Option<SloppyImportsResolution> {
      resolve_with_mode(specifier, ResolutionMode::Types)
    }

    fn resolve_with_mode(
      specifier: &ModuleSpecifier,
      mode: ResolutionMode,
    ) -> Option<SloppyImportsResolution> {
      SloppyImportsResolver::new(Arc::new(deno_fs::RealFs))
        .resolve(specifier, mode)
    }

    let context = TestContext::default();
    let temp_dir = context.temp_dir().path();

    // scenarios like resolving ./example.js to ./example.ts
    for (ext_from, ext_to) in [("js", "ts"), ("js", "tsx"), ("mjs", "mts")] {
      let ts_file = temp_dir.join(format!("file.{}", ext_to));
      ts_file.write("");
      assert_eq!(resolve(&ts_file.uri_file()), None);
      assert_eq!(
        resolve(
          &temp_dir
            .uri_dir()
            .join(&format!("file.{}", ext_from))
            .unwrap()
        ),
        Some(SloppyImportsResolution::JsToTs(ts_file.uri_file())),
      );
      ts_file.remove_file();
    }

    // no extension scenarios
    for ext in ["js", "ts", "js", "tsx", "jsx", "mjs", "mts"] {
      let file = temp_dir.join(format!("file.{}", ext));
      file.write("");
      assert_eq!(
        resolve(
          &temp_dir
            .uri_dir()
            .join("file") // no ext
            .unwrap()
        ),
        Some(SloppyImportsResolution::NoExtension(file.uri_file()))
      );
      file.remove_file();
    }

    // .ts and .js exists, .js specified (goes to specified)
    {
      let ts_file = temp_dir.join("file.ts");
      ts_file.write("");
      let js_file = temp_dir.join("file.js");
      js_file.write("");
      assert_eq!(resolve(&js_file.uri_file()), None);
    }

    // only js exists, .js specified
    {
      let js_only_file = temp_dir.join("js_only.js");
      js_only_file.write("");
      assert_eq!(resolve(&js_only_file.uri_file()), None);
      assert_eq!(resolve_types(&js_only_file.uri_file()), None);
    }

    // resolving a directory to an index file
    {
      let routes_dir = temp_dir.join("routes");
      routes_dir.create_dir_all();
      let index_file = routes_dir.join("index.ts");
      index_file.write("");
      assert_eq!(
        resolve(&routes_dir.uri_file()),
        Some(SloppyImportsResolution::Directory(index_file.uri_file())),
      );
    }

    // both a directory and a file with specifier is present
    {
      let api_dir = temp_dir.join("api");
      api_dir.create_dir_all();
      let bar_file = api_dir.join("bar.ts");
      bar_file.write("");
      let api_file = temp_dir.join("api.ts");
      api_file.write("");
      assert_eq!(
        resolve(&api_dir.uri_file()),
        Some(SloppyImportsResolution::NoExtension(api_file.uri_file())),
      );
    }
  }

  #[test]
  fn test_sloppy_import_resolution_suggestion_message() {
    // directory
    assert_eq!(
      SloppyImportsResolution::Directory(
        ModuleSpecifier::parse("file:///dir/index.js").unwrap()
      )
      .as_suggestion_message(),
      "Maybe specify path to 'index.js' file in directory instead"
    );
    // no ext
    assert_eq!(
      SloppyImportsResolution::NoExtension(
        ModuleSpecifier::parse("file:///dir/index.mjs").unwrap()
      )
      .as_suggestion_message(),
      "Maybe add a '.mjs' extension"
    );
    // js to ts
    assert_eq!(
      SloppyImportsResolution::JsToTs(
        ModuleSpecifier::parse("file:///dir/index.mts").unwrap()
      )
      .as_suggestion_message(),
      "Maybe change the extension to '.mts'"
    );
  }
}
