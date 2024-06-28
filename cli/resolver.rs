// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use async_trait::async_trait;
use dashmap::DashMap;
use dashmap::DashSet;
use deno_ast::MediaType;
use deno_config::package_json::PackageJsonDeps;
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
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_runtime::deno_node::parse_npm_pkg_name;
use deno_runtime::deno_node::NodeResolution;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::NpmResolver as DenoNodeNpmResolver;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::fs_util::specifier_to_file_path;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use import_map::ImportMap;
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::args::JsxImportSourceConfig;
use crate::args::PackageJsonDepsProvider;
use crate::args::DENO_DISABLE_PEDANTIC_NODE_WARNINGS;
use crate::colors;
use crate::node::CliNodeCodeTranslator;
use crate::npm::ByonmCliNpmResolver;
use crate::npm::CliNpmResolver;
use crate::npm::InnerCliNpmResolverRef;
use crate::util::sync::AtomicFlag;

pub fn format_range_with_colors(range: &deno_graph::Range) -> String {
  format!(
    "{}:{}:{}",
    colors::cyan(range.specifier.as_str()),
    colors::yellow(&(range.start.line + 1).to_string()),
    colors::yellow(&(range.start.character + 1).to_string())
  )
}

pub struct ModuleCodeStringSource {
  pub code: ModuleSourceCode,
  pub found_url: ModuleSpecifier,
  pub media_type: MediaType,
}

#[derive(Debug)]
pub struct CliNodeResolver {
  // not used in the LSP
  cjs_resolutions: Option<Arc<CjsResolutionStore>>,
  fs: Arc<dyn deno_fs::FileSystem>,
  node_resolver: Arc<NodeResolver>,
  // todo(dsherret): remove this pub(crate)
  pub(crate) npm_resolver: Arc<dyn CliNpmResolver>,
}

impl CliNodeResolver {
  pub fn new(
    cjs_resolutions: Option<Arc<CjsResolutionStore>>,
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
  ) -> Result<Option<Arc<PackageJson>>, AnyError> {
    self.node_resolver.get_closest_package_json(referrer)
  }

  pub fn resolve_if_in_npm_package(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Option<Result<Option<NodeResolution>, AnyError>> {
    if self.in_npm_package(referrer) {
      // we're in an npm package, so use node resolution
      Some(self.resolve(specifier, referrer, mode))
    } else {
      None
    }
  }

  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<Option<NodeResolution>, AnyError> {
    self.handle_node_resolve_result(
      self.node_resolver.resolve(specifier, referrer, mode),
    )
  }

  pub fn resolve_req_reference(
    &self,
    req_ref: &NpmPackageReqReference,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<NodeResolution, AnyError> {
    let package_folder = self
      .npm_resolver
      .resolve_pkg_folder_from_deno_module_req(req_ref.req(), referrer)?;
    let maybe_resolution = self.resolve_package_sub_path_from_deno_module(
      &package_folder,
      req_ref.sub_path(),
      referrer,
      mode,
    )?;
    match maybe_resolution {
      Some(resolution) => Ok(resolution),
      None => {
        if self.npm_resolver.as_byonm().is_some() {
          let package_json_path = package_folder.join("package.json");
          if !self.fs.exists_sync(&package_json_path) {
            return Err(anyhow!(
              "Could not find '{}'. Deno expects the node_modules/ directory to be up to date. Did you forget to run `npm install`?",
              package_json_path.display()
            ));
          }
        }
        Err(anyhow!(
          "Failed resolving package subpath for '{}' in '{}'.",
          req_ref,
          package_folder.display()
        ))
      }
    }
  }

  pub fn resolve_package_sub_path_from_deno_module(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<Option<NodeResolution>, AnyError> {
    self.handle_node_resolve_result(
      self.node_resolver.resolve_package_subpath_from_deno_module(
        package_folder,
        sub_path,
        referrer,
        mode,
      ),
    )
  }

  pub fn handle_if_in_node_modules(
    &self,
    specifier: ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    // skip canonicalizing if we definitely know it's unnecessary
    if specifier.scheme() == "file"
      && specifier.path().contains("/node_modules/")
    {
      // Specifiers in the node_modules directory are canonicalized
      // so canoncalize then check if it's in the node_modules directory.
      // If so, check if we need to store this specifier as being a CJS
      // resolution.
      let specifier =
        crate::node::resolve_specifier_into_node_modules(&specifier);
      if self.in_npm_package(&specifier) {
        if let Some(cjs_resolutions) = &self.cjs_resolutions {
          let resolution =
            self.node_resolver.url_to_node_resolution(specifier)?;
          if let NodeResolution::CommonJs(specifier) = &resolution {
            cjs_resolutions.insert(specifier.clone());
          }
          return Ok(resolution.into_url());
        } else {
          return Ok(specifier);
        }
      }
    }

    Ok(specifier)
  }

  pub fn url_to_node_resolution(
    &self,
    specifier: ModuleSpecifier,
  ) -> Result<NodeResolution, AnyError> {
    self.node_resolver.url_to_node_resolution(specifier)
  }

  fn handle_node_resolve_result(
    &self,
    result: Result<Option<NodeResolution>, AnyError>,
  ) -> Result<Option<NodeResolution>, AnyError> {
    match result? {
      Some(response) => {
        if let NodeResolution::CommonJs(specifier) = &response {
          // remember that this was a common js resolution
          if let Some(cjs_resolutions) = &self.cjs_resolutions {
            cjs_resolutions.insert(specifier.clone());
          }
        }
        Ok(Some(response))
      }
      None => Ok(None),
    }
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

/// Result of checking if a specifier is mapped via
/// an import map or package.json.
pub enum MappedResolution {
  None,
  PackageJson(ModuleSpecifier),
  ImportMap(ModuleSpecifier),
}

impl MappedResolution {
  pub fn into_specifier(self) -> Option<ModuleSpecifier> {
    match self {
      MappedResolution::None => Option::None,
      MappedResolution::PackageJson(specifier) => Some(specifier),
      MappedResolution::ImportMap(specifier) => Some(specifier),
    }
  }
}

/// Resolver for specifiers that could be mapped via an
/// import map or package.json.
#[derive(Debug)]
pub struct MappedSpecifierResolver {
  maybe_import_map: Option<Arc<ImportMap>>,
  package_json_deps_provider: Arc<PackageJsonDepsProvider>,
}

impl MappedSpecifierResolver {
  pub fn new(
    maybe_import_map: Option<Arc<ImportMap>>,
    package_json_deps_provider: Arc<PackageJsonDepsProvider>,
  ) -> Self {
    Self {
      maybe_import_map,
      package_json_deps_provider,
    }
  }

  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<MappedResolution, AnyError> {
    // attempt to resolve with the import map first
    let maybe_import_map_err = match self
      .maybe_import_map
      .as_ref()
      .map(|import_map| import_map.resolve(specifier, referrer))
    {
      Some(Ok(value)) => return Ok(MappedResolution::ImportMap(value)),
      Some(Err(err)) => Some(err),
      None => None,
    };

    // then with package.json
    if let Some(deps) = self.package_json_deps_provider.deps() {
      if let Some(specifier) = resolve_package_json_dep(specifier, deps)? {
        return Ok(MappedResolution::PackageJson(specifier));
      }
    }

    // otherwise, surface the import map error or try resolving when has no import map
    if let Some(err) = maybe_import_map_err {
      Err(err.into())
    } else {
      Ok(MappedResolution::None)
    }
  }
}

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct CliGraphResolver {
  sloppy_imports_resolver: Option<SloppyImportsResolver>,
  mapped_specifier_resolver: MappedSpecifierResolver,
  maybe_default_jsx_import_source: Option<String>,
  maybe_default_jsx_import_source_types: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
  maybe_vendor_specifier: Option<ModuleSpecifier>,
  node_resolver: Option<Arc<CliNodeResolver>>,
  npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  found_package_json_dep_flag: AtomicFlag,
  bare_node_builtins_enabled: bool,
}

pub struct CliGraphResolverOptions<'a> {
  pub sloppy_imports_resolver: Option<SloppyImportsResolver>,
  pub node_resolver: Option<Arc<CliNodeResolver>>,
  pub npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  pub package_json_deps_provider: Arc<PackageJsonDepsProvider>,
  pub maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub maybe_vendor_dir: Option<&'a PathBuf>,
  pub bare_node_builtins_enabled: bool,
}

impl CliGraphResolver {
  pub fn new(options: CliGraphResolverOptions) -> Self {
    let is_byonm = options
      .npm_resolver
      .as_ref()
      .map(|n| n.as_byonm().is_some())
      .unwrap_or(false);
    Self {
      sloppy_imports_resolver: options.sloppy_imports_resolver,
      mapped_specifier_resolver: MappedSpecifierResolver::new(
        options.maybe_import_map,
        if is_byonm {
          // don't resolve from the root package.json deps for byonm
          Arc::new(PackageJsonDepsProvider::new(None))
        } else {
          options.package_json_deps_provider
        },
      ),
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
      node_resolver: options.node_resolver,
      npm_resolver: options.npm_resolver,
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

  fn check_surface_byonm_node_error(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    original_err: AnyError,
    resolver: &ByonmCliNpmResolver,
  ) -> Result<(), AnyError> {
    if let Ok((pkg_name, _, _)) = parse_npm_pkg_name(specifier, referrer) {
      match resolver.resolve_package_folder_from_package(&pkg_name, referrer) {
        Ok(_) => {
          return Err(original_err);
        }
        Err(_) => {
          if resolver
            .find_ancestor_package_json_with_dep(&pkg_name, referrer)
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
      }
    }
    Ok(())
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
    let result: Result<_, ResolveError> = self
      .mapped_specifier_resolver
      .resolve(specifier, referrer)
      .map_err(|err| err.into())
      .and_then(|resolution| match resolution {
        MappedResolution::ImportMap(specifier) => Ok(specifier),
        MappedResolution::PackageJson(specifier) => {
          // found a specifier in the package.json, so mark that
          // we need to do an "npm install" later
          self.found_package_json_dep_flag.raise();
          Ok(specifier)
        }
        MappedResolution::None => {
          deno_graph::resolve_import(specifier, &referrer_range.specifier)
            .map_err(|err| err.into())
        }
      });

    // do sloppy imports resolution if enabled
    let result =
      if let Some(sloppy_imports_resolver) = &self.sloppy_imports_resolver {
        result.map(|specifier| {
          sloppy_imports_resolve(
            sloppy_imports_resolver,
            specifier,
            referrer_range,
            mode,
          )
        })
      } else {
        result
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

    if let Some(resolver) =
      self.npm_resolver.as_ref().and_then(|r| r.as_byonm())
    {
      match &result {
        Ok(specifier) => {
          if let Ok(npm_req_ref) =
            NpmPackageReqReference::from_specifier(specifier)
          {
            let node_resolver = self.node_resolver.as_ref().unwrap();
            return node_resolver
              .resolve_req_reference(&npm_req_ref, referrer, to_node_mode(mode))
              .map(|res| res.into_url())
              .map_err(|err| err.into());
          }
        }
        Err(_) => {
          if referrer.scheme() == "file" {
            if let Some(node_resolver) = &self.node_resolver {
              let node_result =
                node_resolver.resolve(specifier, referrer, to_node_mode(mode));
              match node_result {
                Ok(Some(res)) => {
                  return Ok(res.into_url());
                }
                Ok(None) => {
                  self
                    .check_surface_byonm_node_error(
                      specifier,
                      referrer,
                      anyhow!("Cannot find \"{}\"", specifier),
                      resolver,
                    )
                    .map_err(ResolveError::Other)?;
                }
                Err(err) => {
                  self
                    .check_surface_byonm_node_error(
                      specifier, referrer, err, resolver,
                    )
                    .map_err(ResolveError::Other)?;
                }
              }
            }
          }
        }
      }
    } else if referrer.scheme() == "file" {
      if let Some(node_resolver) = &self.node_resolver {
        let node_result = node_resolver.resolve_if_in_npm_package(
          specifier,
          referrer,
          to_node_mode(mode),
        );
        if let Some(Ok(Some(res))) = node_result {
          return Ok(res.into_url());
        }
      }
    }

    let specifier = result?;
    match &self.node_resolver {
      Some(node_resolver) => node_resolver
        .handle_if_in_node_modules(specifier)
        .map_err(|e| e.into()),
      None => Ok(specifier),
    }
  }
}

fn sloppy_imports_resolve(
  resolver: &SloppyImportsResolver,
  specifier: ModuleSpecifier,
  referrer_range: &deno_graph::Range,
  mode: ResolutionMode,
) -> ModuleSpecifier {
  let resolution = resolver.resolve(&specifier, mode);
  if mode.is_types() {
    // don't bother warning for types resolution because
    // we already probably warned during execution resolution
    match resolution {
      SloppyImportsResolution::None(_) => return specifier, // avoid a clone
      _ => return resolution.into_specifier().into_owned(),
    }
  }

  let hint_message = match &resolution {
    SloppyImportsResolution::JsToTs(to_specifier) => {
      let to_media_type = MediaType::from_specifier(to_specifier);
      let from_media_type = MediaType::from_specifier(&specifier);
      format!(
        "update {} extension to {}",
        from_media_type.as_ts_extension(),
        to_media_type.as_ts_extension()
      )
    }
    SloppyImportsResolution::NoExtension(to_specifier) => {
      let to_media_type = MediaType::from_specifier(to_specifier);
      format!("add {} extension", to_media_type.as_ts_extension())
    }
    SloppyImportsResolution::Directory(to_specifier) => {
      let file_name = to_specifier
        .path()
        .rsplit_once('/')
        .map(|(_, file_name)| file_name)
        .unwrap_or(to_specifier.path());
      format!("specify path to {} file in directory instead", file_name)
    }
    SloppyImportsResolution::None(_) => return specifier,
  };
  // show a warning when this happens in order to drive
  // the user towards correcting these specifiers
  if !*DENO_DISABLE_PEDANTIC_NODE_WARNINGS {
    log::warn!(
      "{} Sloppy module resolution {}\n    at {}",
      crate::colors::yellow("Warning"),
      crate::colors::gray(format!("(hint: {})", hint_message)).to_string(),
      if referrer_range.end == deno_graph::Position::zeroed() {
        // not worth showing the range in this case
        crate::colors::cyan(referrer_range.specifier.as_str()).to_string()
      } else {
        format_range_with_colors(referrer_range)
      },
    );
  }

  resolution.into_specifier().into_owned()
}

fn resolve_package_json_dep(
  specifier: &str,
  deps: &PackageJsonDeps,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  for (bare_specifier, req_result) in deps {
    if specifier.starts_with(bare_specifier) {
      let path = &specifier[bare_specifier.len()..];
      if path.is_empty() || path.starts_with('/') {
        let req = req_result.as_ref().map_err(|err| {
          anyhow!(
            "Parsing version constraints in the application-level package.json is more strict at the moment.\n\n{:#}",
            err.clone()
          )
        })?;
        return Ok(Some(ModuleSpecifier::parse(&format!("npm:{req}{path}"))?));
      }
    }
  }

  Ok(None)
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
          npm_resolver.ensure_top_level_package_json_install().await
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

#[derive(Debug, PartialEq, Eq)]
pub enum SloppyImportsResolution<'a> {
  /// No sloppy resolution was found.
  None(&'a ModuleSpecifier),
  /// Ex. `./file.js` to `./file.ts`
  JsToTs(ModuleSpecifier),
  /// Ex. `./file` to `./file.ts`
  NoExtension(ModuleSpecifier),
  /// Ex. `./dir` to `./dir/index.ts`
  Directory(ModuleSpecifier),
}

impl<'a> SloppyImportsResolution<'a> {
  pub fn as_specifier(&self) -> &ModuleSpecifier {
    match self {
      Self::None(specifier) => specifier,
      Self::JsToTs(specifier) => specifier,
      Self::NoExtension(specifier) => specifier,
      Self::Directory(specifier) => specifier,
    }
  }

  pub fn into_specifier(self) -> Cow<'a, ModuleSpecifier> {
    match self {
      Self::None(specifier) => Cow::Borrowed(specifier),
      Self::JsToTs(specifier) => Cow::Owned(specifier),
      Self::NoExtension(specifier) => Cow::Owned(specifier),
      Self::Directory(specifier) => Cow::Owned(specifier),
    }
  }

  pub fn as_suggestion_message(&self) -> Option<String> {
    Some(format!("Maybe {}", self.as_base_message()?))
  }

  pub fn as_lsp_quick_fix_message(&self) -> Option<String> {
    let message = self.as_base_message()?;
    let mut chars = message.chars();
    Some(format!(
      "{}{}.",
      chars.next().unwrap().to_uppercase(),
      chars.as_str()
    ))
  }

  fn as_base_message(&self) -> Option<String> {
    match self {
      SloppyImportsResolution::None(_) => None,
      SloppyImportsResolution::JsToTs(specifier) => {
        let media_type = MediaType::from_specifier(specifier);
        Some(format!(
          "change the extension to '{}'",
          media_type.as_ts_extension()
        ))
      }
      SloppyImportsResolution::NoExtension(specifier) => {
        let media_type = MediaType::from_specifier(specifier);
        Some(format!(
          "add a '{}' extension",
          media_type.as_ts_extension()
        ))
      }
      SloppyImportsResolution::Directory(specifier) => {
        let file_name = specifier
          .path()
          .rsplit_once('/')
          .map(|(_, file_name)| file_name)
          .unwrap_or(specifier.path());
        Some(format!(
          "specify path to '{}' file in directory instead",
          file_name
        ))
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

  pub fn resolve<'a>(
    &self,
    specifier: &'a ModuleSpecifier,
    mode: ResolutionMode,
  ) -> SloppyImportsResolution<'a> {
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
      probe_media_type_types: Vec<MediaType>,
      reason: SloppyImportsResolutionReason,
    ) -> Vec<(PathBuf, SloppyImportsResolutionReason)> {
      probe_media_type_types
        .into_iter()
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
      return SloppyImportsResolution::None(specifier);
    }

    let Ok(path) = specifier_to_file_path(specifier) else {
      return SloppyImportsResolution::None(specifier);
    };

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
              _ => return SloppyImportsResolution::None(specifier),
            };
            let Some(path_no_ext) = path_without_ext(&path, media_type) else {
              return SloppyImportsResolution::None(specifier);
            };
            media_types_to_paths(
              &path_no_ext,
              probe_media_type_types,
              SloppyImportsResolutionReason::JsToTs,
            )
          } else {
            return SloppyImportsResolution::None(specifier);
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
              return SloppyImportsResolution::None(specifier)
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
            return SloppyImportsResolution::None(specifier);
          }
          probe_paths
        }
      };

    for (probe_path, reason) in probe_paths {
      if self.stat_sync(&probe_path) == Some(SloppyImportsFsEntry::File) {
        if let Ok(specifier) = ModuleSpecifier::from_file_path(probe_path) {
          match reason {
            SloppyImportsResolutionReason::JsToTs => {
              return SloppyImportsResolution::JsToTs(specifier)
            }
            SloppyImportsResolutionReason::NoExtension => {
              return SloppyImportsResolution::NoExtension(specifier)
            }
            SloppyImportsResolutionReason::Directory => {
              return SloppyImportsResolution::Directory(specifier)
            }
          }
        }
      }
    }

    SloppyImportsResolution::None(specifier)
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
  use std::collections::BTreeMap;

  use test_util::TestContext;

  use super::*;

  #[test]
  fn test_resolve_package_json_dep() {
    fn resolve(
      specifier: &str,
      deps: &BTreeMap<String, PackageReq>,
    ) -> Result<Option<String>, String> {
      let deps = deps
        .iter()
        .map(|(key, value)| (key.to_string(), Ok(value.clone())))
        .collect();
      resolve_package_json_dep(specifier, &deps)
        .map(|s| s.map(|s| s.to_string()))
        .map_err(|err| err.to_string())
    }

    let deps = BTreeMap::from([
      (
        "package".to_string(),
        PackageReq::from_str("package@1.0").unwrap(),
      ),
      (
        "package-alias".to_string(),
        PackageReq::from_str("package@^1.2").unwrap(),
      ),
      (
        "@deno/test".to_string(),
        PackageReq::from_str("@deno/test@~0.2").unwrap(),
      ),
    ]);

    assert_eq!(
      resolve("package", &deps).unwrap(),
      Some("npm:package@1.0".to_string()),
    );
    assert_eq!(
      resolve("package/some_path.ts", &deps).unwrap(),
      Some("npm:package@1.0/some_path.ts".to_string()),
    );

    assert_eq!(
      resolve("@deno/test", &deps).unwrap(),
      Some("npm:@deno/test@~0.2".to_string()),
    );
    assert_eq!(
      resolve("@deno/test/some_path.ts", &deps).unwrap(),
      Some("npm:@deno/test@~0.2/some_path.ts".to_string()),
    );
    // matches the start, but doesn't have the same length or a path
    assert_eq!(resolve("@deno/testing", &deps).unwrap(), None,);

    // alias
    assert_eq!(
      resolve("package-alias", &deps).unwrap(),
      Some("npm:package@^1.2".to_string()),
    );

    // non-existent bare specifier
    assert_eq!(resolve("non-existent", &deps).unwrap(), None);
  }

  #[test]
  fn test_unstable_sloppy_imports() {
    fn resolve(specifier: &ModuleSpecifier) -> SloppyImportsResolution {
      SloppyImportsResolver::new(Arc::new(deno_fs::RealFs))
        .resolve(specifier, ResolutionMode::Execution)
    }

    let context = TestContext::default();
    let temp_dir = context.temp_dir().path();

    // scenarios like resolving ./example.js to ./example.ts
    for (ext_from, ext_to) in [("js", "ts"), ("js", "tsx"), ("mjs", "mts")] {
      let ts_file = temp_dir.join(format!("file.{}", ext_to));
      ts_file.write("");
      let ts_file_uri = ts_file.uri_file();
      assert_eq!(
        resolve(&ts_file.uri_file()),
        SloppyImportsResolution::None(&ts_file_uri),
      );
      assert_eq!(
        resolve(
          &temp_dir
            .uri_dir()
            .join(&format!("file.{}", ext_from))
            .unwrap()
        ),
        SloppyImportsResolution::JsToTs(ts_file.uri_file()),
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
        SloppyImportsResolution::NoExtension(file.uri_file()),
      );
      file.remove_file();
    }

    // .ts and .js exists, .js specified (goes to specified)
    {
      let ts_file = temp_dir.join("file.ts");
      ts_file.write("");
      let js_file = temp_dir.join("file.js");
      js_file.write("");
      let js_file_uri = js_file.uri_file();
      assert_eq!(
        resolve(&js_file.uri_file()),
        SloppyImportsResolution::None(&js_file_uri),
      );
    }

    // resolving a directory to an index file
    {
      let routes_dir = temp_dir.join("routes");
      routes_dir.create_dir_all();
      let index_file = routes_dir.join("index.ts");
      index_file.write("");
      assert_eq!(
        resolve(&routes_dir.uri_file()),
        SloppyImportsResolution::Directory(index_file.uri_file()),
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
        SloppyImportsResolution::NoExtension(api_file.uri_file()),
      );
    }
  }

  #[test]
  fn test_sloppy_import_resolution_suggestion_message() {
    // none
    let url = ModuleSpecifier::parse("file:///dir/index.js").unwrap();
    assert_eq!(
      SloppyImportsResolution::None(&url).as_suggestion_message(),
      None,
    );
    // directory
    assert_eq!(
      SloppyImportsResolution::Directory(
        ModuleSpecifier::parse("file:///dir/index.js").unwrap()
      )
      .as_suggestion_message()
      .unwrap(),
      "Maybe specify path to 'index.js' file in directory instead"
    );
    // no ext
    assert_eq!(
      SloppyImportsResolution::NoExtension(
        ModuleSpecifier::parse("file:///dir/index.mjs").unwrap()
      )
      .as_suggestion_message()
      .unwrap(),
      "Maybe add a '.mjs' extension"
    );
    // js to ts
    assert_eq!(
      SloppyImportsResolution::JsToTs(
        ModuleSpecifier::parse("file:///dir/index.mts").unwrap()
      )
      .as_suggestion_message()
      .unwrap(),
      "Maybe change the extension to '.mts'"
    );
  }
}
