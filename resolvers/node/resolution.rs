// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Error as AnyError;
use dashmap::DashMap;
use deno_media_type::MediaType;
use deno_package_json::PackageJson;
use deno_path_util::url_to_file_path;
use deno_semver::Version;
use deno_semver::VersionReq;
use serde_json::Map;
use serde_json::Value;
use sys_traits::FileType;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use sys_traits::FsRead;
use url::Url;

use crate::cache::NodeResolutionSys;
use crate::errors;
use crate::errors::DataUrlReferrerError;
use crate::errors::FinalizeResolutionError;
use crate::errors::InvalidModuleSpecifierError;
use crate::errors::InvalidPackageTargetError;
use crate::errors::LegacyResolveError;
use crate::errors::ModuleNotFoundError;
use crate::errors::NodeJsErrorCode;
use crate::errors::NodeJsErrorCoded;
use crate::errors::NodeResolveError;
use crate::errors::NodeResolveRelativeJoinError;
use crate::errors::PackageExportsResolveError;
use crate::errors::PackageImportNotDefinedError;
use crate::errors::PackageImportsResolveError;
use crate::errors::PackageImportsResolveErrorKind;
use crate::errors::PackagePathNotExportedError;
use crate::errors::PackageResolveError;
use crate::errors::PackageSubpathResolveError;
use crate::errors::PackageSubpathResolveErrorKind;
use crate::errors::PackageTargetNotFoundError;
use crate::errors::PackageTargetResolveError;
use crate::errors::PackageTargetResolveErrorKind;
use crate::errors::ResolveBinaryCommandsError;
use crate::errors::ResolvePkgJsonBinExportError;
use crate::errors::TypesNotFoundError;
use crate::errors::TypesNotFoundErrorData;
use crate::errors::UnsupportedDirImportError;
use crate::errors::UnsupportedEsmUrlSchemeError;
use crate::path::UrlOrPath;
use crate::path::UrlOrPathRef;
use crate::InNpmPackageChecker;
use crate::IsBuiltInNodeModuleChecker;
use crate::NpmPackageFolderResolver;
use crate::PackageJsonResolverRc;
use crate::PathClean;

pub static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];
pub static REQUIRE_CONDITIONS: &[&str] = &["require", "node"];
static TYPES_ONLY_CONDITIONS: &[&str] = &["types"];

#[allow(clippy::disallowed_types)]
type ConditionsFromResolutionModeFn = crate::sync::MaybeArc<
  dyn Fn(ResolutionMode) -> &'static [&'static str] + Send + Sync + 'static,
>;

#[derive(Default, Clone)]
pub struct ConditionsFromResolutionMode(Option<ConditionsFromResolutionModeFn>);

impl Debug for ConditionsFromResolutionMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ConditionsFromResolutionMode").finish()
  }
}

impl ConditionsFromResolutionMode {
  pub fn new(func: ConditionsFromResolutionModeFn) -> Self {
    Self(Some(func))
  }

  fn resolve(
    &self,
    resolution_mode: ResolutionMode,
  ) -> &'static [&'static str] {
    match &self.0 {
      Some(func) => func(ResolutionMode::Import),
      None => resolution_mode.default_conditions(),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResolutionMode {
  Import,
  Require,
}

impl ResolutionMode {
  pub fn default_conditions(&self) -> &'static [&'static str] {
    match self {
      ResolutionMode::Import => DEFAULT_CONDITIONS,
      ResolutionMode::Require => REQUIRE_CONDITIONS,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeResolutionKind {
  Execution,
  Types,
}

impl NodeResolutionKind {
  pub fn is_types(&self) -> bool {
    matches!(self, NodeResolutionKind::Types)
  }
}

#[derive(Debug)]
pub enum NodeResolution {
  Module(UrlOrPath),
  BuiltIn(String),
}

impl NodeResolution {
  pub fn into_url(self) -> Result<Url, NodeResolveError> {
    match self {
      Self::Module(u) => Ok(u.into_url()?),
      Self::BuiltIn(specifier) => Ok(if specifier.starts_with("node:") {
        Url::parse(&specifier).unwrap()
      } else {
        Url::parse(&format!("node:{specifier}")).unwrap()
      }),
    }
  }
}

struct LocalPath {
  path: PathBuf,
  known_exists: bool,
}

enum LocalUrlOrPath {
  Url(Url),
  Path(LocalPath),
}

impl LocalUrlOrPath {
  pub fn into_url_or_path(self) -> UrlOrPath {
    match self {
      LocalUrlOrPath::Url(url) => UrlOrPath::Url(url),
      LocalUrlOrPath::Path(local_path) => UrlOrPath::Path(local_path.path),
    }
  }
}

/// This struct helps ensure we remember to probe for
/// declaration files and to prevent accidentally probing
/// multiple times.
struct MaybeTypesResolvedUrl(LocalUrlOrPath);

/// Kind of method that resolution suceeded with.
enum ResolvedMethod {
  Url,
  RelativeOrAbsolute,
  PackageImports,
  PackageExports,
  PackageSubPath,
}

#[derive(Debug, Default, Clone)]
pub struct NodeResolverOptions {
  pub conditions_from_resolution_mode: ConditionsFromResolutionMode,
  /// TypeScript version to use for typesVersions resolution and
  /// `types@req` exports resolution.
  pub typescript_version: Option<Version>,
}

#[allow(clippy::disallowed_types)]
pub type NodeResolverRc<
  TInNpmPackageChecker,
  TIsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver,
  TSys,
> = crate::sync::MaybeArc<
  NodeResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
>;

#[derive(Debug)]
pub struct NodeResolver<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: FsCanonicalize + FsMetadata + FsRead,
> {
  in_npm_pkg_checker: TInNpmPackageChecker,
  is_built_in_node_module_checker: TIsBuiltInNodeModuleChecker,
  npm_pkg_folder_resolver: TNpmPackageFolderResolver,
  pkg_json_resolver: PackageJsonResolverRc<TSys>,
  sys: NodeResolutionSys<TSys>,
  conditions_from_resolution_mode: ConditionsFromResolutionMode,
  typescript_version: Option<Version>,
  package_resolution_lookup_cache: Option<DashMap<Url, String>>,
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: FsCanonicalize + FsMetadata + FsRead,
  >
  NodeResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  pub fn new(
    in_npm_pkg_checker: TInNpmPackageChecker,
    is_built_in_node_module_checker: TIsBuiltInNodeModuleChecker,
    npm_pkg_folder_resolver: TNpmPackageFolderResolver,
    pkg_json_resolver: PackageJsonResolverRc<TSys>,
    sys: NodeResolutionSys<TSys>,
    options: NodeResolverOptions,
  ) -> Self {
    Self {
      in_npm_pkg_checker,
      is_built_in_node_module_checker,
      npm_pkg_folder_resolver,
      pkg_json_resolver,
      sys,
      conditions_from_resolution_mode: options.conditions_from_resolution_mode,
      typescript_version: options.typescript_version,
      package_resolution_lookup_cache: None,
    }
  }

  pub fn with_package_resolution_lookup_cache(self) -> Self {
    Self {
      package_resolution_lookup_cache: Some(Default::default()),
      ..self
    }
  }

  pub fn in_npm_package(&self, specifier: &Url) -> bool {
    self.in_npm_pkg_checker.in_npm_package(specifier)
  }

  /// This function is an implementation of `defaultResolve` in
  /// `lib/internal/modules/esm/resolve.js` from Node.
  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<NodeResolution, NodeResolveError> {
    // Note: if we are here, then the referrer is an esm module
    // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

    if self
      .is_built_in_node_module_checker
      .is_builtin_node_module(specifier)
    {
      return Ok(NodeResolution::BuiltIn(specifier.to_string()));
    }

    let mut specifier_is_url = false;
    if let Ok(url) = Url::parse(specifier) {
      specifier_is_url = true;
      if url.scheme() == "data" {
        return Ok(NodeResolution::Module(UrlOrPath::Url(url)));
      }

      if let Some(module_name) =
        get_module_name_from_builtin_node_module_specifier(&url)
      {
        return Ok(NodeResolution::BuiltIn(module_name.to_string()));
      }

      let protocol = url.scheme();

      if protocol != "file" && protocol != "data" {
        return Err(
          UnsupportedEsmUrlSchemeError {
            url_scheme: protocol.to_string(),
          }
          .into(),
        );
      }

      // todo(dsherret): this seems wrong
      if referrer.scheme() == "data" {
        let url = referrer
          .join(specifier)
          .map_err(|source| DataUrlReferrerError { source })?;
        return Ok(NodeResolution::Module(UrlOrPath::Url(url)));
      }
    }

    let conditions = self
      .conditions_from_resolution_mode
      .resolve(resolution_mode);
    let referrer = UrlOrPathRef::from_url(referrer);
    let (url, resolved_kind) = self.module_resolve(
      specifier,
      &referrer,
      resolution_mode,
      conditions,
      resolution_kind,
    )?;

    let url_or_path =
      self.finalize_resolution(url, resolved_kind, Some(&referrer))?;
    let maybe_cache_resolution = || {
      let package_resolution_lookup_cache =
        self.package_resolution_lookup_cache.as_ref()?;
      if specifier_is_url
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with("/")
      {
        return None;
      }
      let url = url_or_path.clone().into_url().ok()?;
      package_resolution_lookup_cache.insert(url, specifier.to_string());
      Some(())
    };
    maybe_cache_resolution();
    let resolve_response = NodeResolution::Module(url_or_path);
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(resolve_response)
  }

  fn module_resolve(
    &self,
    specifier: &str,
    referrer: &UrlOrPathRef,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), NodeResolveError> {
    if should_be_treated_as_relative_or_absolute_path(specifier) {
      let referrer_url = referrer.url()?;
      let url = node_join_url(referrer_url, specifier).map_err(|err| {
        NodeResolveRelativeJoinError {
          path: specifier.to_string(),
          base: referrer_url.clone(),
          source: err,
        }
      })?;
      let url = self.maybe_resolve_types(
        LocalUrlOrPath::Url(url),
        Some(referrer),
        resolution_mode,
        conditions,
        resolution_kind,
      )?;
      Ok((url, ResolvedMethod::RelativeOrAbsolute))
    } else if specifier.starts_with('#') {
      let pkg_config = self
        .pkg_json_resolver
        .get_closest_package_json(referrer.path()?)
        .map_err(PackageImportsResolveErrorKind::ClosestPkgJson)
        .map_err(|err| PackageImportsResolveError(Box::new(err)))?;
      Ok((
        self.package_imports_resolve_internal(
          specifier,
          Some(referrer),
          resolution_mode,
          pkg_config.as_deref(),
          conditions,
          resolution_kind,
        )?,
        ResolvedMethod::PackageImports,
      ))
    } else if let Ok(url) = Url::parse(specifier) {
      let url_or_path = self.maybe_resolve_types(
        LocalUrlOrPath::Url(url),
        Some(referrer),
        resolution_mode,
        conditions,
        resolution_kind,
      )?;
      Ok((url_or_path, ResolvedMethod::Url))
    } else {
      Ok(self.package_resolve(
        specifier,
        referrer,
        resolution_mode,
        conditions,
        resolution_kind,
      )?)
    }
  }

  fn finalize_resolution(
    &self,
    resolved: MaybeTypesResolvedUrl,
    resolved_method: ResolvedMethod,
    maybe_referrer: Option<&UrlOrPathRef>,
  ) -> Result<UrlOrPath, FinalizeResolutionError> {
    let encoded_sep_re = lazy_regex::regex!(r"%2F|%2C");

    let resolved = resolved.0;
    let text = match &resolved {
      LocalUrlOrPath::Url(url) => Cow::Borrowed(url.as_str()),
      LocalUrlOrPath::Path(LocalPath { path, .. }) => path.to_string_lossy(),
    };
    if encoded_sep_re.is_match(&text) {
      return Err(
        errors::InvalidModuleSpecifierError {
          request: text.into_owned(),
          reason: Cow::Borrowed(
            "must not include encoded \"/\" or \"\\\\\" characters",
          ),
          maybe_referrer: maybe_referrer.map(|r| match r.path() {
            // in this case, prefer showing the path string
            Ok(path) => path.display().to_string(),
            Err(_) => r.display().to_string(),
          }),
        }
        .into(),
      );
    }

    let (path, maybe_url) = match resolved {
      LocalUrlOrPath::Url(url) => {
        if url.scheme() == "file" {
          (url_to_file_path(&url)?, Some(url))
        } else {
          return Ok(UrlOrPath::Url(url));
        }
      }
      LocalUrlOrPath::Path(LocalPath { path, known_exists }) => {
        if known_exists {
          // no need to do the finalization checks
          return Ok(UrlOrPath::Path(path));
        } else {
          (path, None)
        }
      }
    };

    // TODO(bartlomieju): currently not supported
    // if (getOptionValue('--experimental-specifier-resolution') === 'node') {
    //   ...
    // }

    let p_str = path.to_str().unwrap();
    let path = if p_str.ends_with('/') {
      PathBuf::from(&p_str[p_str.len() - 1..])
    } else {
      path
    };

    let maybe_file_type = self.sys.get_file_type(&path);
    match maybe_file_type {
      Ok(FileType::Dir) => {
        let suggested_file_name = ["index.mjs", "index.js", "index.cjs"]
          .into_iter()
          .find(|e| self.sys.is_file(&path.join(e)));
        Err(
          UnsupportedDirImportError {
            dir_url: UrlOrPath::Path(path),
            maybe_referrer: maybe_referrer.map(|r| r.display()),
            suggested_file_name,
          }
          .into(),
        )
      }
      Ok(FileType::File) => {
        // prefer returning the url to avoid re-allocating in the CLI crate
        Ok(
          maybe_url
            .map(UrlOrPath::Url)
            .unwrap_or(UrlOrPath::Path(path)),
        )
      }
      _ => Err(
        ModuleNotFoundError {
          suggested_ext: self
            .module_not_found_ext_suggestion(&path, resolved_method),
          specifier: UrlOrPath::Path(path),
          maybe_referrer: maybe_referrer.map(|r| r.display()),
          typ: "module",
        }
        .into(),
      ),
    }
  }

  pub fn lookup_package_specifier_for_resolution(
    &self,
    url: &Url,
  ) -> Option<String> {
    self
      .package_resolution_lookup_cache
      .as_ref()?
      .get(url)
      .map(|r| r.value().clone())
  }

  fn module_not_found_ext_suggestion(
    &self,
    path: &Path,
    resolved_method: ResolvedMethod,
  ) -> Option<&'static str> {
    fn should_probe(path: &Path, resolved_method: ResolvedMethod) -> bool {
      if MediaType::from_path(path) != MediaType::Unknown {
        return false;
      }
      match resolved_method {
        ResolvedMethod::Url
        | ResolvedMethod::RelativeOrAbsolute
        | ResolvedMethod::PackageSubPath => true,
        ResolvedMethod::PackageImports | ResolvedMethod::PackageExports => {
          false
        }
      }
    }

    if should_probe(path, resolved_method) {
      ["js", "mjs", "cjs"]
        .into_iter()
        .find(|ext| self.sys.is_file(&with_known_extension(path, ext)))
    } else {
      None
    }
  }

  pub fn resolve_package_subpath_from_deno_module(
    &self,
    package_dir: &Path,
    package_subpath: Option<&str>,
    maybe_referrer: Option<&Url>,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<UrlOrPath, PackageSubpathResolveError> {
    // todo(dsherret): don't allocate a string here (maybe use an
    // enum that says the subpath is not prefixed with a ./)
    let package_subpath = package_subpath
      .map(|s| format!("./{s}"))
      .unwrap_or_else(|| ".".to_string());
    let maybe_referrer = maybe_referrer.map(UrlOrPathRef::from_url);
    let (resolved_url, resolved_method) = self.resolve_package_dir_subpath(
      package_dir,
      &package_subpath,
      maybe_referrer.as_ref(),
      resolution_mode,
      self
        .conditions_from_resolution_mode
        .resolve(resolution_mode),
      resolution_kind,
    )?;
    let url_or_path = self.finalize_resolution(
      resolved_url,
      resolved_method,
      maybe_referrer.as_ref(),
    )?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(url_or_path)
  }

  pub fn resolve_binary_commands(
    &self,
    package_folder: &Path,
  ) -> Result<Vec<String>, ResolveBinaryCommandsError> {
    let pkg_json_path = package_folder.join("package.json");
    let Some(package_json) =
      self.pkg_json_resolver.load_package_json(&pkg_json_path)?
    else {
      return Ok(Vec::new());
    };

    Ok(match &package_json.bin {
      Some(Value::String(_)) => {
        let Some(name) = &package_json.name else {
          return Err(ResolveBinaryCommandsError::MissingPkgJsonName {
            pkg_json_path,
          });
        };
        let name = name.split("/").last().unwrap();
        vec![name.to_string()]
      }
      Some(Value::Object(o)) => {
        o.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>()
      }
      _ => Vec::new(),
    })
  }

  pub fn resolve_binary_export(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
  ) -> Result<PathBuf, ResolvePkgJsonBinExportError> {
    let pkg_json_path = package_folder.join("package.json");
    let Some(package_json) =
      self.pkg_json_resolver.load_package_json(&pkg_json_path)?
    else {
      return Err(ResolvePkgJsonBinExportError::MissingPkgJson {
        pkg_json_path,
      });
    };
    let bin_entry =
      resolve_bin_entry_value(&package_json, sub_path).map_err(|err| {
        ResolvePkgJsonBinExportError::InvalidBinProperty {
          message: err.to_string(),
        }
      })?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(package_folder.join(bin_entry))
  }

  /// Resolves an npm package folder path from the specified referrer.
  pub fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, errors::PackageFolderResolveError> {
    self
      .npm_pkg_folder_resolver
      .resolve_package_folder_from_package(specifier, referrer)
  }

  fn maybe_resolve_types(
    &self,
    url: LocalUrlOrPath,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, TypesNotFoundError> {
    if resolution_kind.is_types() {
      let file_path = match url {
        LocalUrlOrPath::Url(url) => {
          match deno_path_util::url_to_file_path(&url) {
            Ok(path) => LocalPath {
              path,
              known_exists: false,
            },
            Err(_) => {
              return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Url(url)));
            }
          }
        }
        LocalUrlOrPath::Path(path) => path,
      };
      self.path_to_declaration_path(
        file_path,
        maybe_referrer,
        resolution_mode,
        conditions,
      )
    } else {
      Ok(MaybeTypesResolvedUrl(url))
    }
  }

  /// Checks if the resolved file has a corresponding declaration file.
  fn path_to_declaration_path(
    &self,
    local_path: LocalPath,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
  ) -> Result<MaybeTypesResolvedUrl, TypesNotFoundError> {
    fn probe_extensions<TSys: FsMetadata>(
      sys: &NodeResolutionSys<TSys>,
      path: &Path,
      media_type: MediaType,
      resolution_mode: ResolutionMode,
    ) -> Option<PathBuf> {
      let mut searched_for_d_mts = false;
      let mut searched_for_d_cts = false;
      if media_type == MediaType::Mjs {
        let d_mts_path = with_known_extension(path, "d.mts");
        if sys.exists_(&d_mts_path) {
          return Some(d_mts_path);
        }
        searched_for_d_mts = true;
      } else if media_type == MediaType::Cjs {
        let d_cts_path = with_known_extension(path, "d.cts");
        if sys.exists_(&d_cts_path) {
          return Some(d_cts_path);
        }
        searched_for_d_cts = true;
      }

      let dts_path = with_known_extension(path, "d.ts");
      if sys.exists_(&dts_path) {
        return Some(dts_path);
      }

      let specific_dts_path = match resolution_mode {
        ResolutionMode::Require if !searched_for_d_cts => {
          Some(with_known_extension(path, "d.cts"))
        }
        ResolutionMode::Import if !searched_for_d_mts => {
          Some(with_known_extension(path, "d.mts"))
        }
        _ => None, // already searched above
      };
      if let Some(specific_dts_path) = specific_dts_path {
        if sys.exists_(&specific_dts_path) {
          return Some(specific_dts_path);
        }
      }
      let ts_path = with_known_extension(path, "ts");
      if sys.is_file(&ts_path) {
        return Some(ts_path);
      }
      None
    }

    let media_type = MediaType::from_path(&local_path.path);
    if media_type.is_declaration() {
      return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Path(local_path)));
    }
    if let Some(path) =
      probe_extensions(&self.sys, &local_path.path, media_type, resolution_mode)
    {
      return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Path(LocalPath {
        path,
        known_exists: true,
      })));
    }
    if self.sys.is_dir(&local_path.path) {
      let resolution_result = self.resolve_package_dir_subpath(
        &local_path.path,
        /* sub path */ ".",
        maybe_referrer,
        resolution_mode,
        conditions,
        NodeResolutionKind::Types,
      );
      if let Ok((url_or_path, _)) = resolution_result {
        return Ok(url_or_path);
      }
      let index_path = local_path.path.join("index.js");
      if let Some(path) = probe_extensions(
        &self.sys,
        &index_path,
        MediaType::from_path(&index_path),
        resolution_mode,
      ) {
        return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Path(LocalPath {
          path,
          known_exists: true,
        })));
      }
    }
    // allow resolving .ts-like or .css files for types resolution
    if media_type.is_typed() || media_type == MediaType::Css {
      return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Path(local_path)));
    }
    Err(TypesNotFoundError(Box::new(TypesNotFoundErrorData {
      code_specifier: UrlOrPathRef::from_path(&local_path.path).display(),
      maybe_referrer: maybe_referrer.map(|r| r.display()),
    })))
  }

  #[allow(clippy::too_many_arguments)]
  pub fn package_imports_resolve(
    &self,
    name: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    referrer_pkg_json: Option<&PackageJson>,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<UrlOrPath, PackageImportsResolveError> {
    self
      .package_imports_resolve_internal(
        name,
        maybe_referrer,
        resolution_mode,
        referrer_pkg_json,
        conditions,
        resolution_kind,
      )
      .map(|url| url.0.into_url_or_path())
  }

  #[allow(clippy::too_many_arguments)]
  fn package_imports_resolve_internal(
    &self,
    name: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    referrer_pkg_json: Option<&PackageJson>,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, PackageImportsResolveError> {
    if name == "#" || name.starts_with("#/") || name.ends_with('/') {
      let reason = "is not a valid internal imports specifier name";
      return Err(
        errors::InvalidModuleSpecifierError {
          request: name.to_string(),
          reason: Cow::Borrowed(reason),
          maybe_referrer: maybe_referrer.map(to_specifier_display_string),
        }
        .into(),
      );
    }

    let mut package_json_path = None;
    if let Some(pkg_json) = &referrer_pkg_json {
      package_json_path = Some(pkg_json.path.clone());
      if let Some(imports) = &pkg_json.imports {
        if imports.contains_key(name) && !name.contains('*') {
          let target = imports.get(name).unwrap();
          let maybe_resolved = self.resolve_package_target(
            package_json_path.as_ref().unwrap(),
            target,
            "",
            name,
            maybe_referrer,
            resolution_mode,
            false,
            true,
            conditions,
            resolution_kind,
          )?;
          if let Some(resolved) = maybe_resolved {
            return Ok(resolved);
          }
        } else {
          let mut best_match = "";
          let mut best_match_subpath = None;
          for key in imports.keys() {
            let pattern_index = key.find('*');
            if let Some(pattern_index) = pattern_index {
              let key_sub = &key[0..pattern_index];
              if name.starts_with(key_sub) {
                let pattern_trailer = &key[pattern_index + 1..];
                if name.len() > key.len()
                  && name.ends_with(&pattern_trailer)
                  && pattern_key_compare(best_match, key) == 1
                  && key.rfind('*') == Some(pattern_index)
                {
                  best_match = key;
                  best_match_subpath = Some(
                    &name[pattern_index..(name.len() - pattern_trailer.len())],
                  );
                }
              }
            }
          }

          if !best_match.is_empty() {
            let target = imports.get(best_match).unwrap();
            let maybe_resolved = self.resolve_package_target(
              package_json_path.as_ref().unwrap(),
              target,
              best_match_subpath.unwrap(),
              best_match,
              maybe_referrer,
              resolution_mode,
              true,
              true,
              conditions,
              resolution_kind,
            )?;
            if let Some(resolved) = maybe_resolved {
              return Ok(resolved);
            }
          }
        }
      }
    }

    Err(
      PackageImportNotDefinedError {
        name: name.to_string(),
        package_json_path,
        maybe_referrer: maybe_referrer.map(|r| r.display()),
      }
      .into(),
    )
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_target_string(
    &self,
    target: &str,
    subpath: &str,
    match_: &str,
    package_json_path: &Path,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, PackageTargetResolveError> {
    if !subpath.is_empty() && !pattern && !target.ends_with('/') {
      return Err(
        InvalidPackageTargetError {
          pkg_json_path: package_json_path.to_path_buf(),
          sub_path: match_.to_string(),
          target: target.to_string(),
          is_import: internal,
          maybe_referrer: maybe_referrer.map(|r| r.display()),
        }
        .into(),
      );
    }
    let invalid_segment_re =
      lazy_regex::regex!(r"(^|\\|/)(\.\.?|node_modules)(\\|/|$)");
    let pattern_re = lazy_regex::regex!(r"\*");
    if !target.starts_with("./") {
      if internal && !target.starts_with("../") && !target.starts_with('/') {
        let target_url = Url::parse(target);
        match target_url {
          Ok(url) => {
            if get_module_name_from_builtin_node_module_specifier(&url)
              .is_some()
            {
              return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Url(url)));
            }
          }
          Err(_) => {
            let export_target = if pattern {
              pattern_re
                .replace(target, |_caps: &regex::Captures| subpath)
                .to_string()
            } else {
              format!("{target}{subpath}")
            };
            let result = match self.package_resolve(
              &export_target,
              &UrlOrPathRef::from_path(package_json_path),
              resolution_mode,
              conditions,
              resolution_kind,
            ) {
              Ok((url, _)) => Ok(url),
              Err(err) => match err.code() {
                NodeJsErrorCode::ERR_INVALID_FILE_URL_PATH
                | NodeJsErrorCode::ERR_INVALID_MODULE_SPECIFIER
                | NodeJsErrorCode::ERR_INVALID_PACKAGE_CONFIG
                | NodeJsErrorCode::ERR_INVALID_PACKAGE_TARGET
                | NodeJsErrorCode::ERR_PACKAGE_IMPORT_NOT_DEFINED
                | NodeJsErrorCode::ERR_PACKAGE_PATH_NOT_EXPORTED
                | NodeJsErrorCode::ERR_UNKNOWN_FILE_EXTENSION
                | NodeJsErrorCode::ERR_UNSUPPORTED_DIR_IMPORT
                | NodeJsErrorCode::ERR_UNSUPPORTED_ESM_URL_SCHEME
                | NodeJsErrorCode::ERR_TYPES_NOT_FOUND => {
                  Err(PackageTargetResolveErrorKind::PackageResolve(err).into())
                }
                NodeJsErrorCode::ERR_MODULE_NOT_FOUND => Err(
                  PackageTargetResolveErrorKind::NotFound(
                    PackageTargetNotFoundError {
                      pkg_json_path: package_json_path.to_path_buf(),
                      target: export_target.to_string(),
                      maybe_referrer: maybe_referrer.map(|r| r.display()),
                      resolution_mode,
                      resolution_kind,
                    },
                  )
                  .into(),
                ),
              },
            };

            return match result {
              Ok(url) => Ok(url),
              Err(err) => {
                if self
                  .is_built_in_node_module_checker
                  .is_builtin_node_module(target)
                {
                  Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Url(
                    Url::parse(&format!("node:{}", target)).unwrap(),
                  )))
                } else {
                  Err(err)
                }
              }
            };
          }
        }
      }
      return Err(
        InvalidPackageTargetError {
          pkg_json_path: package_json_path.to_path_buf(),
          sub_path: match_.to_string(),
          target: target.to_string(),
          is_import: internal,
          maybe_referrer: maybe_referrer.map(|r| r.display()),
        }
        .into(),
      );
    }
    if invalid_segment_re.is_match(&target[2..]) {
      return Err(
        InvalidPackageTargetError {
          pkg_json_path: package_json_path.to_path_buf(),
          sub_path: match_.to_string(),
          target: target.to_string(),
          is_import: internal,
          maybe_referrer: maybe_referrer.map(|r| r.display()),
        }
        .into(),
      );
    }
    let package_path = package_json_path.parent().unwrap();
    let resolved_path = package_path.join(target).clean();
    if !resolved_path.starts_with(package_path) {
      return Err(
        InvalidPackageTargetError {
          pkg_json_path: package_json_path.to_path_buf(),
          sub_path: match_.to_string(),
          target: target.to_string(),
          is_import: internal,
          maybe_referrer: maybe_referrer.map(|r| r.display()),
        }
        .into(),
      );
    }
    let path = if subpath.is_empty() {
      LocalPath {
        path: resolved_path,
        known_exists: false,
      }
    } else if invalid_segment_re.is_match(subpath) {
      let request = if pattern {
        match_.replace('*', subpath)
      } else {
        format!("{match_}{subpath}")
      };
      return Err(
        throw_invalid_subpath(
          request,
          package_json_path,
          internal,
          maybe_referrer,
        )
        .into(),
      );
    } else if pattern {
      let resolved_path_str = resolved_path.to_string_lossy();
      let replaced = pattern_re
        .replace(&resolved_path_str, |_caps: &regex::Captures| subpath);
      LocalPath {
        path: PathBuf::from(replaced.as_ref()),
        known_exists: false,
      }
    } else {
      LocalPath {
        path: resolved_path.join(subpath).clean(),
        known_exists: false,
      }
    };
    Ok(self.maybe_resolve_types(
      LocalUrlOrPath::Path(path),
      maybe_referrer,
      resolution_mode,
      conditions,
      resolution_kind,
    )?)
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_target(
    &self,
    package_json_path: &Path,
    target: &Value,
    subpath: &str,
    package_subpath: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<Option<MaybeTypesResolvedUrl>, PackageTargetResolveError> {
    let result = self.resolve_package_target_inner(
      package_json_path,
      target,
      subpath,
      package_subpath,
      maybe_referrer,
      resolution_mode,
      pattern,
      internal,
      conditions,
      resolution_kind,
    );
    match result {
      Ok(maybe_resolved) => Ok(maybe_resolved),
      Err(err) => {
        if resolution_kind.is_types()
          && err.code() == NodeJsErrorCode::ERR_TYPES_NOT_FOUND
          && conditions != TYPES_ONLY_CONDITIONS
        {
          // try resolving with just "types" conditions for when someone misconfigures
          // and puts the "types" condition in the wrong place
          if let Ok(Some(resolved)) = self.resolve_package_target_inner(
            package_json_path,
            target,
            subpath,
            package_subpath,
            maybe_referrer,
            resolution_mode,
            pattern,
            internal,
            TYPES_ONLY_CONDITIONS,
            resolution_kind,
          ) {
            return Ok(Some(resolved));
          }
        }

        Err(err)
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_target_inner(
    &self,
    package_json_path: &Path,
    target: &Value,
    subpath: &str,
    package_subpath: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<Option<MaybeTypesResolvedUrl>, PackageTargetResolveError> {
    if let Some(target) = target.as_str() {
      let url_or_path = self.resolve_package_target_string(
        target,
        subpath,
        package_subpath,
        package_json_path,
        maybe_referrer,
        resolution_mode,
        pattern,
        internal,
        conditions,
        resolution_kind,
      )?;
      return Ok(Some(url_or_path));
    } else if let Some(target_arr) = target.as_array() {
      if target_arr.is_empty() {
        return Ok(None);
      }

      let mut last_error = None;
      for target_item in target_arr {
        let resolved_result = self.resolve_package_target(
          package_json_path,
          target_item,
          subpath,
          package_subpath,
          maybe_referrer,
          resolution_mode,
          pattern,
          internal,
          conditions,
          resolution_kind,
        );

        match resolved_result {
          Ok(Some(resolved)) => return Ok(Some(resolved)),
          Ok(None) => {
            last_error = None;
            continue;
          }
          Err(e) => {
            if e.code() == NodeJsErrorCode::ERR_INVALID_PACKAGE_TARGET {
              last_error = Some(e);
              continue;
            } else {
              return Err(e);
            }
          }
        }
      }
      if last_error.is_none() {
        return Ok(None);
      }
      return Err(last_error.unwrap());
    } else if let Some(target_obj) = target.as_object() {
      for (key, condition_target) in target_obj {
        // TODO(bartlomieju): verify that keys are not numeric
        // return Err(errors::err_invalid_package_config(
        //   to_file_path_string(package_json_url),
        //   Some(base.as_str().to_string()),
        //   Some("\"exports\" cannot contain numeric property keys.".to_string()),
        // ));

        if key == "default"
          || conditions.contains(&key.as_str())
          || resolution_kind.is_types() && self.matches_types_key(key)
        {
          let resolved = self.resolve_package_target(
            package_json_path,
            condition_target,
            subpath,
            package_subpath,
            maybe_referrer,
            resolution_mode,
            pattern,
            internal,
            conditions,
            resolution_kind,
          )?;
          match resolved {
            Some(resolved) => return Ok(Some(resolved)),
            None => {
              continue;
            }
          }
        }
      }
    } else if target.is_null() {
      return Ok(None);
    }

    Err(
      InvalidPackageTargetError {
        pkg_json_path: package_json_path.to_path_buf(),
        sub_path: package_subpath.to_string(),
        target: target.to_string(),
        is_import: internal,
        maybe_referrer: maybe_referrer.map(|r| r.display()),
      }
      .into(),
    )
  }

  fn matches_types_key(&self, key: &str) -> bool {
    if key == "types" {
      return true;
    }
    let Some(ts_version) = &self.typescript_version else {
      return false;
    };
    let Some(constraint) = key.strip_prefix("types@") else {
      return false;
    };
    let Ok(version_req) = VersionReq::parse_from_npm(constraint) else {
      return false;
    };
    version_req.matches(ts_version)
  }

  #[allow(clippy::too_many_arguments)]
  pub fn package_exports_resolve(
    &self,
    package_json_path: &Path,
    package_subpath: &str,
    package_exports: &Map<String, Value>,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<UrlOrPath, PackageExportsResolveError> {
    self
      .package_exports_resolve_internal(
        package_json_path,
        package_subpath,
        package_exports,
        maybe_referrer,
        resolution_mode,
        conditions,
        resolution_kind,
      )
      .map(|url| url.0.into_url_or_path())
  }

  #[allow(clippy::too_many_arguments)]
  fn package_exports_resolve_internal(
    &self,
    package_json_path: &Path,
    package_subpath: &str,
    package_exports: &Map<String, Value>,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, PackageExportsResolveError> {
    if let Some(target) = package_exports.get(package_subpath) {
      if package_subpath.find('*').is_none() && !package_subpath.ends_with('/')
      {
        let resolved = self.resolve_package_target(
          package_json_path,
          target,
          "",
          package_subpath,
          maybe_referrer,
          resolution_mode,
          false,
          false,
          conditions,
          resolution_kind,
        )?;
        return match resolved {
          Some(resolved) => Ok(resolved),
          None => Err(
            PackagePathNotExportedError {
              pkg_json_path: package_json_path.to_path_buf(),
              subpath: package_subpath.to_string(),
              maybe_referrer: maybe_referrer.map(|r| r.display()),
              resolution_kind,
            }
            .into(),
          ),
        };
      }
    }

    let mut best_match = "";
    let mut best_match_data = None;
    for (key, target) in package_exports {
      let Some(pattern_index) = key.find('*') else {
        continue;
      };
      let key_sub = &key[0..pattern_index];
      if !package_subpath.starts_with(key_sub) {
        continue;
      }

      // When this reaches EOL, this can throw at the top of the whole function:
      //
      // if (StringPrototypeEndsWith(packageSubpath, '/'))
      //   throwInvalidSubpath(packageSubpath)
      //
      // To match "imports" and the spec.
      if package_subpath.ends_with('/') {
        // TODO(bartlomieju):
        // emitTrailingSlashPatternDeprecation();
      }
      let pattern_trailer = &key[pattern_index + 1..];
      if package_subpath.len() >= key.len()
        && package_subpath.ends_with(&pattern_trailer)
        && pattern_key_compare(best_match, key) == 1
        && key.rfind('*') == Some(pattern_index)
      {
        best_match = key;
        best_match_data = Some((
          target,
          &package_subpath
            [pattern_index..(package_subpath.len() - pattern_trailer.len())],
        ));
      }
    }

    if let Some((target, subpath)) = best_match_data {
      let maybe_resolved = self.resolve_package_target(
        package_json_path,
        target,
        subpath,
        best_match,
        maybe_referrer,
        resolution_mode,
        true,
        false,
        conditions,
        resolution_kind,
      )?;
      if let Some(resolved) = maybe_resolved {
        return Ok(resolved);
      } else {
        return Err(
          PackagePathNotExportedError {
            pkg_json_path: package_json_path.to_path_buf(),
            subpath: package_subpath.to_string(),
            maybe_referrer: maybe_referrer.map(|r| r.display()),
            resolution_kind,
          }
          .into(),
        );
      }
    }

    Err(
      PackagePathNotExportedError {
        pkg_json_path: package_json_path.to_path_buf(),
        subpath: package_subpath.to_string(),
        maybe_referrer: maybe_referrer.map(|r| r.display()),
        resolution_kind,
      }
      .into(),
    )
  }

  fn package_resolve(
    &self,
    specifier: &str,
    referrer: &UrlOrPathRef,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageResolveError> {
    let (package_name, package_subpath, _is_scoped) =
      parse_npm_pkg_name(specifier, referrer)?;

    if let Some(package_config) = self
      .pkg_json_resolver
      .get_closest_package_json(referrer.path()?)?
    {
      // ResolveSelf
      if package_config.name.as_deref() == Some(package_name) {
        if let Some(exports) = &package_config.exports {
          return self
            .package_exports_resolve_internal(
              &package_config.path,
              &package_subpath,
              exports,
              Some(referrer),
              resolution_mode,
              conditions,
              resolution_kind,
            )
            .map(|url| (url, ResolvedMethod::PackageExports))
            .map_err(|err| err.into());
        }
      }
    }

    self.resolve_package_subpath_for_package(
      package_name,
      &package_subpath,
      referrer,
      resolution_mode,
      conditions,
      resolution_kind,
    )
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_subpath_for_package(
    &self,
    package_name: &str,
    package_subpath: &str,
    referrer: &UrlOrPathRef,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageResolveError> {
    let result = self.resolve_package_subpath_for_package_inner(
      package_name,
      package_subpath,
      referrer,
      resolution_mode,
      conditions,
      resolution_kind,
    );
    if resolution_kind.is_types() && result.is_err() {
      // try to resolve with the @types package
      let package_name = types_package_name(package_name);
      if let Ok(result) = self.resolve_package_subpath_for_package_inner(
        &package_name,
        package_subpath,
        referrer,
        resolution_mode,
        conditions,
        resolution_kind,
      ) {
        return Ok(result);
      }
    }
    result
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_subpath_for_package_inner(
    &self,
    package_name: &str,
    package_subpath: &str,
    referrer: &UrlOrPathRef,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageResolveError> {
    let package_dir_path = self
      .npm_pkg_folder_resolver
      .resolve_package_folder_from_package(package_name, referrer)?;

    // todo: error with this instead when can't find package
    // Err(errors::err_module_not_found(
    //   &package_json_url
    //     .join(".")
    //     .unwrap()
    //     .to_file_path()
    //     .unwrap()
    //     .display()
    //     .to_string(),
    //   &to_file_path_string(referrer),
    //   "package",
    // ))

    // Package match.
    self
      .resolve_package_dir_subpath(
        &package_dir_path,
        package_subpath,
        Some(referrer),
        resolution_mode,
        conditions,
        resolution_kind,
      )
      .map_err(|err| err.into())
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_dir_subpath(
    &self,
    package_dir_path: &Path,
    package_subpath: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageSubpathResolveError>
  {
    let package_json_path = package_dir_path.join("package.json");
    match self
      .pkg_json_resolver
      .load_package_json(&package_json_path)?
    {
      Some(pkg_json) => self.resolve_package_subpath(
        &pkg_json,
        package_subpath,
        maybe_referrer,
        resolution_mode,
        conditions,
        resolution_kind,
      ),
      None => self
        .resolve_package_subpath_no_pkg_json(
          package_dir_path,
          package_subpath,
          maybe_referrer,
          resolution_mode,
          conditions,
          resolution_kind,
        )
        .map(|url| (url, ResolvedMethod::PackageSubPath))
        .map_err(|err| {
          PackageSubpathResolveErrorKind::LegacyResolve(err).into()
        }),
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_subpath(
    &self,
    package_json: &PackageJson,
    package_subpath: &str,
    referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageSubpathResolveError>
  {
    if let Some(exports) = &package_json.exports {
      let result = self.package_exports_resolve_internal(
        &package_json.path,
        package_subpath,
        exports,
        referrer,
        resolution_mode,
        conditions,
        resolution_kind,
      );
      match result {
        Ok(found) => return Ok((found, ResolvedMethod::PackageExports)),
        Err(exports_err) => {
          if resolution_kind.is_types() && package_subpath == "." {
            return self
              .legacy_main_resolve(
                package_json,
                referrer,
                resolution_mode,
                conditions,
                resolution_kind,
              )
              .map(|url| (url, ResolvedMethod::PackageSubPath))
              .map_err(|err| {
                PackageSubpathResolveErrorKind::LegacyResolve(err).into()
              });
          }
          return Err(
            PackageSubpathResolveErrorKind::Exports(exports_err).into(),
          );
        }
      }
    }

    if package_subpath == "." {
      self
        .legacy_main_resolve(
          package_json,
          referrer,
          resolution_mode,
          conditions,
          resolution_kind,
        )
        .map(|url| (url, ResolvedMethod::PackageSubPath))
        .map_err(|err| {
          PackageSubpathResolveErrorKind::LegacyResolve(err).into_box()
        })
    } else {
      self
        .resolve_subpath_exact(
          package_json.path.parent().unwrap(),
          package_subpath,
          Some(package_json),
          referrer,
          resolution_mode,
          conditions,
          resolution_kind,
        )
        .map(|url| (url, ResolvedMethod::PackageSubPath))
        .map_err(|err| {
          PackageSubpathResolveErrorKind::LegacyResolve(err.into()).into_box()
        })
    }
  }

  fn pkg_json_types_versions<'a>(
    &'a self,
    pkg_json: &'a PackageJson,
    resolution_kind: NodeResolutionKind,
  ) -> Option<TypesVersions<'a, TSys>> {
    if !resolution_kind.is_types() {
      return None;
    }
    pkg_json
      .types_versions
      .as_ref()
      .and_then(|entries| {
        let ts_version = self.typescript_version.as_ref()?;
        entries
          .iter()
          .filter_map(|(k, v)| {
            let version_req = VersionReq::parse_from_npm(k).ok()?;
            version_req.matches(ts_version).then_some(v)
          })
          .next()
      })
      .and_then(|value| value.as_object())
      .map(|value| TypesVersions {
        value,
        dir_path: pkg_json.dir_path(),
        sys: &self.sys,
      })
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_subpath_exact(
    &self,
    directory: &Path,
    package_subpath: &str,
    package_json: Option<&PackageJson>,
    referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, TypesNotFoundError> {
    assert_ne!(package_subpath, ".");
    let types_versions = package_json.and_then(|pkg_json| {
      self.pkg_json_types_versions(pkg_json, resolution_kind)
    });
    let package_subpath = types_versions
      .and_then(|v| v.map(package_subpath))
      .unwrap_or(Cow::Borrowed(package_subpath));
    let file_path = directory.join(package_subpath.as_ref());
    self.maybe_resolve_types(
      LocalUrlOrPath::Path(LocalPath {
        path: file_path,
        known_exists: false,
      }),
      referrer,
      resolution_mode,
      conditions,
      resolution_kind,
    )
  }

  fn resolve_package_subpath_no_pkg_json(
    &self,
    directory: &Path,
    package_subpath: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, LegacyResolveError> {
    if package_subpath == "." {
      self.legacy_index_resolve(
        directory,
        maybe_referrer,
        resolution_mode,
        resolution_kind,
      )
    } else {
      self
        .resolve_subpath_exact(
          directory,
          package_subpath,
          None,
          maybe_referrer,
          resolution_mode,
          conditions,
          resolution_kind,
        )
        .map_err(|err| err.into())
    }
  }

  fn legacy_main_resolve(
    &self,
    package_json: &PackageJson,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[&str],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, LegacyResolveError> {
    let pkg_json_kind = match resolution_mode {
      ResolutionMode::Require => deno_package_json::NodeModuleKind::Cjs,
      ResolutionMode::Import => deno_package_json::NodeModuleKind::Esm,
    };

    let maybe_main = if resolution_kind.is_types() {
      match package_json.types.as_ref() {
        Some(types) => {
          let types_versions =
            self.pkg_json_types_versions(package_json, resolution_kind);
          Some(
            types_versions
              .and_then(|v| v.map(types.as_ref()))
              .unwrap_or(Cow::Borrowed(types.as_str())),
          )
        }
        None => {
          // fallback to checking the main entrypoint for
          // a corresponding declaration file
          if let Some(main) = package_json.main(pkg_json_kind) {
            let main = package_json.path.parent().unwrap().join(main).clean();
            let decl_path_result = self.path_to_declaration_path(
              LocalPath {
                path: main,
                known_exists: false,
              },
              maybe_referrer,
              resolution_mode,
              conditions,
            );
            // don't surface errors, fallback to checking the index now
            if let Ok(url_or_path) = decl_path_result {
              return Ok(url_or_path);
            }
          }
          None
        }
      }
    } else {
      package_json.main(pkg_json_kind).map(Cow::Borrowed)
    };

    if let Some(main) = maybe_main.as_deref() {
      let guess = package_json.path.parent().unwrap().join(main).clean();
      if self.sys.is_file(&guess) {
        return Ok(self.maybe_resolve_types(
          LocalUrlOrPath::Path(LocalPath {
            path: guess,
            known_exists: true,
          }),
          maybe_referrer,
          resolution_mode,
          conditions,
          resolution_kind,
        )?);
      }

      // todo(dsherret): investigate exactly how node and typescript handles this
      let endings = if resolution_kind.is_types() {
        match resolution_mode {
          ResolutionMode::Require => {
            vec![".d.ts", ".d.cts", "/index.d.ts", "/index.d.cts"]
          }
          ResolutionMode::Import => vec![
            ".d.ts",
            ".d.mts",
            "/index.d.ts",
            "/index.d.mts",
            ".d.cts",
            "/index.d.cts",
          ],
        }
      } else {
        vec![".js", "/index.js"]
      };
      for ending in endings {
        let guess = package_json
          .path
          .parent()
          .unwrap()
          .join(format!("{main}{ending}"))
          .clean();
        if self.sys.is_file(&guess) {
          // TODO(bartlomieju): emitLegacyIndexDeprecation()
          return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Path(LocalPath {
            path: guess,
            known_exists: true,
          })));
        }
      }
    }

    self.legacy_index_resolve(
      package_json.path.parent().unwrap(),
      maybe_referrer,
      resolution_mode,
      resolution_kind,
    )
  }

  fn legacy_index_resolve(
    &self,
    directory: &Path,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, LegacyResolveError> {
    let index_file_names = if resolution_kind.is_types() {
      // todo(dsherret): investigate exactly how typescript does this
      match resolution_mode {
        ResolutionMode::Require => vec!["index.d.ts", "index.d.cts"],
        ResolutionMode::Import => {
          vec!["index.d.ts", "index.d.mts", "index.d.cts"]
        }
      }
    } else {
      vec!["index.js"]
    };
    for index_file_name in index_file_names {
      let guess = directory.join(index_file_name).clean();
      if self.sys.is_file(&guess) {
        // TODO(bartlomieju): emitLegacyIndexDeprecation()
        return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Path(LocalPath {
          path: guess,
          known_exists: true,
        })));
      }
    }

    if resolution_kind.is_types() {
      Err(
        TypesNotFoundError(Box::new(TypesNotFoundErrorData {
          code_specifier: UrlOrPathRef::from_path(&directory.join("index.js"))
            .display(),
          maybe_referrer: maybe_referrer.map(|r| r.display()),
        }))
        .into(),
      )
    } else {
      Err(
        ModuleNotFoundError {
          specifier: UrlOrPath::Path(directory.join("index.js")),
          typ: "module",
          maybe_referrer: maybe_referrer.map(|r| r.display()),
          suggested_ext: None,
        }
        .into(),
      )
    }
  }

  /// Resolves a specifier that is pointing into a node_modules folder by canonicalizing it.
  ///
  /// Returns `None` when the specifier is not in a node_modules folder.
  pub fn handle_if_in_node_modules(&self, specifier: &Url) -> Option<Url> {
    // skip canonicalizing if we definitely know it's unnecessary
    if specifier.scheme() == "file"
      && specifier.path().contains("/node_modules/")
    {
      // Specifiers in the node_modules directory are canonicalized
      // so canoncalize then check if it's in the node_modules directory.
      let specifier = resolve_specifier_into_node_modules(&self.sys, specifier);
      return Some(specifier);
    }

    None
  }
}

fn resolve_bin_entry_value<'a>(
  package_json: &'a PackageJson,
  bin_name: Option<&str>,
) -> Result<&'a str, AnyError> {
  let bin = match &package_json.bin {
    Some(bin) => bin,
    None => bail!(
      "'{}' did not have a bin property",
      package_json.path.display(),
    ),
  };
  let bin_entry = match bin {
    Value::String(_) => {
      if bin_name.is_some()
        && bin_name
          != package_json
            .name
            .as_deref()
            .map(|name| name.rsplit_once('/').map_or(name, |(_, name)| name))
      {
        None
      } else {
        Some(bin)
      }
    }
    Value::Object(o) => {
      if let Some(bin_name) = bin_name {
        o.get(bin_name)
      } else if o.len() == 1
        || o.len() > 1 && o.values().all(|v| v == o.values().next().unwrap())
      {
        o.values().next()
      } else {
        package_json.name.as_ref().and_then(|n| o.get(n))
      }
    }
    _ => bail!(
      "'{}' did not have a bin property with a string or object value",
      package_json.path.display()
    ),
  };
  let bin_entry = match bin_entry {
    Some(e) => e,
    None => {
      let prefix = package_json
        .name
        .as_ref()
        .map(|n| {
          let mut prefix = format!("npm:{}", n);
          if let Some(version) = &package_json.version {
            prefix.push('@');
            prefix.push_str(version);
          }
          prefix.push('/');
          prefix
        })
        .unwrap_or_default();
      let keys = bin
        .as_object()
        .map(|o| {
          o.keys()
            .map(|k| format!(" * {prefix}{k}"))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
      bail!(
        "'{}' did not have a bin entry{}{}",
        package_json.path.display(),
        bin_name
          .or(package_json.name.as_deref())
          .map(|name| format!(" for '{}'", name))
          .unwrap_or_default(),
        if keys.is_empty() {
          "".to_string()
        } else {
          format!("\n\nPossibilities:\n{}", keys.join("\n"))
        }
      )
    }
  };
  match bin_entry {
    Value::String(s) => Ok(s),
    _ => bail!(
      "'{}' had a non-string sub property of bin",
      package_json.path.display(),
    ),
  }
}

fn should_be_treated_as_relative_or_absolute_path(specifier: &str) -> bool {
  if specifier.is_empty() {
    return false;
  }

  if specifier.starts_with('/') {
    return true;
  }

  deno_path_util::is_relative_specifier(specifier)
}

/// Alternate `PathBuf::with_extension` that will handle known extensions
/// more intelligently.
fn with_known_extension(path: &Path, ext: &str) -> PathBuf {
  const NON_DECL_EXTS: &[&str] = &[
    "cjs", "js", "json", "jsx", "mjs", "tsx", /* ex. types.d */ "d",
  ];
  const DECL_EXTS: &[&str] = &["cts", "mts", "ts"];

  let file_name = match path.file_name() {
    Some(value) => value.to_string_lossy(),
    None => return path.to_path_buf(),
  };
  let lowercase_file_name = file_name.to_lowercase();
  let period_index = lowercase_file_name.rfind('.').and_then(|period_index| {
    let ext = &lowercase_file_name[period_index + 1..];
    if DECL_EXTS.contains(&ext) {
      if let Some(next_period_index) =
        lowercase_file_name[..period_index].rfind('.')
      {
        if &lowercase_file_name[next_period_index + 1..period_index] == "d" {
          Some(next_period_index)
        } else {
          Some(period_index)
        }
      } else {
        Some(period_index)
      }
    } else if NON_DECL_EXTS.contains(&ext) {
      Some(period_index)
    } else {
      None
    }
  });

  let file_name = match period_index {
    Some(period_index) => &file_name[..period_index],
    None => &file_name,
  };
  path.with_file_name(format!("{file_name}.{ext}"))
}

fn to_specifier_display_string(url: &UrlOrPathRef) -> String {
  if let Ok(path) = url.path() {
    path.display().to_string()
  } else {
    url.display().to_string()
  }
}

fn throw_invalid_subpath(
  subpath: String,
  package_json_path: &Path,
  internal: bool,
  maybe_referrer: Option<&UrlOrPathRef>,
) -> InvalidModuleSpecifierError {
  let ie = if internal { "imports" } else { "exports" };
  let reason = format!(
    "request is not a valid subpath for the \"{}\" resolution of {}",
    ie,
    package_json_path.display(),
  );
  InvalidModuleSpecifierError {
    request: subpath,
    reason: Cow::Owned(reason),
    maybe_referrer: maybe_referrer.map(to_specifier_display_string),
  }
}

pub fn parse_npm_pkg_name<'a>(
  specifier: &'a str,
  referrer: &UrlOrPathRef,
) -> Result<(&'a str, Cow<'static, str>, bool), InvalidModuleSpecifierError> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = true;
  let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else if specifier.starts_with('@') {
    is_scoped = true;
    if let Some(index) = separator_index {
      separator_index = specifier[index + 1..]
        .find('/')
        .map(|new_index| index + 1 + new_index);
    } else {
      valid_package_name = false;
    }
  }

  let (package_name, subpath) = if let Some(index) = separator_index {
    let (package_name, subpath) = specifier.split_at(index);
    (package_name, Cow::Owned(format!(".{}", subpath)))
  } else {
    (specifier, Cow::Borrowed("."))
  };

  // Package name cannot have leading . and cannot have percent-encoding or separators.
  for ch in package_name.chars() {
    if ch == '%' || ch == '\\' {
      valid_package_name = false;
      break;
    }
  }

  if !valid_package_name {
    return Err(errors::InvalidModuleSpecifierError {
      request: specifier.to_string(),
      reason: Cow::Borrowed("is not a valid package name"),
      maybe_referrer: Some(to_specifier_display_string(referrer)),
    });
  }

  Ok((package_name, subpath, is_scoped))
}

/// Resolves a specifier that is pointing into a node_modules folder.
///
/// Note: This should be called whenever getting the specifier from
/// a Module::External(module) reference because that module might
/// not be fully resolved at the time deno_graph is analyzing it
/// because the node_modules folder might not exist at that time.
pub fn resolve_specifier_into_node_modules(
  sys: &impl FsCanonicalize,
  specifier: &Url,
) -> Url {
  deno_path_util::url_to_file_path(specifier)
    .ok()
    // this path might not exist at the time the graph is being created
    // because the node_modules folder might not yet exist
    .and_then(|path| {
      deno_path_util::fs::canonicalize_path_maybe_not_exists(sys, &path).ok()
    })
    .and_then(|path| deno_path_util::url_from_file_path(&path).ok())
    .unwrap_or_else(|| specifier.clone())
}

fn pattern_key_compare(a: &str, b: &str) -> i32 {
  let a_pattern_index = a.find('*');
  let b_pattern_index = b.find('*');

  let base_len_a = if let Some(index) = a_pattern_index {
    index + 1
  } else {
    a.len()
  };
  let base_len_b = if let Some(index) = b_pattern_index {
    index + 1
  } else {
    b.len()
  };

  if base_len_a > base_len_b {
    return -1;
  }

  if base_len_b > base_len_a {
    return 1;
  }

  if a_pattern_index.is_none() {
    return 1;
  }

  if b_pattern_index.is_none() {
    return -1;
  }

  if a.len() > b.len() {
    return -1;
  }

  if b.len() > a.len() {
    return 1;
  }

  0
}

/// Gets the corresponding @types package for the provided package name.
pub fn types_package_name(package_name: &str) -> String {
  debug_assert!(!package_name.starts_with("@types/"));
  // Scoped packages will get two underscores for each slash
  // https://github.com/DefinitelyTyped/DefinitelyTyped/tree/15f1ece08f7b498f4b9a2147c2a46e94416ca777#what-about-scoped-packages
  format!(
    "@types/{}",
    package_name.trim_start_matches('@').replace('/', "__")
  )
}

/// Ex. returns `fs` for `node:fs`
fn get_module_name_from_builtin_node_module_specifier(
  specifier: &Url,
) -> Option<&str> {
  if specifier.scheme() != "node" {
    return None;
  }

  let (_, specifier) = specifier.as_str().split_once(':')?;
  Some(specifier)
}

/// Node is more lenient joining paths than the url crate is,
/// so this function handles that.
fn node_join_url(url: &Url, path: &str) -> Result<Url, url::ParseError> {
  if let Some(suffix) = path.strip_prefix(".//") {
    // specifier had two leading slashes
    url.join(&format!("./{}", suffix))
  } else {
    url.join(path)
  }
}

struct TypesVersions<'a, TSys: FsMetadata> {
  dir_path: &'a Path,
  value: &'a serde_json::Map<std::string::String, serde_json::Value>,
  sys: &'a NodeResolutionSys<TSys>,
}

impl<'a, TSys: FsMetadata> TypesVersions<'a, TSys> {
  pub fn map(&self, search: &str) -> Option<Cow<'a, str>> {
    let mut search = search
      .strip_prefix("./")
      .unwrap_or(search)
      .trim_matches('/');
    for (key, value) in self.value {
      let key = key.strip_suffix("./").unwrap_or(key).trim_matches('/');
      let is_match = if key == "*" || key == search {
        true
      } else if let Some(key_prefix) = key.strip_suffix("/*") {
        if let Some(new_search) = search.strip_prefix(key_prefix) {
          search = new_search.trim_matches('/');
          true
        } else {
          false
        }
      } else {
        false
      };
      if !is_match {
        continue;
      }
      if let Some(values) = value.as_array() {
        for value in values.iter().filter_map(|s| s.as_str()) {
          let value = if let Some(asterisk_index) = value.find('*') {
            Cow::Owned(format!(
              "{}{}{}",
              &value[..asterisk_index],
              search,
              &value[asterisk_index + 1..]
            ))
          } else {
            Cow::Borrowed(value)
          };
          let path = self.dir_path.join(value.as_ref());
          if self.sys.is_file(&path) {
            return Some(value);
          }
        }
      }
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;
  use sys_traits::impls::InMemorySys;
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsWrite;

  use super::*;

  fn build_package_json(json: Value) -> PackageJson {
    PackageJson::load_from_value(PathBuf::from("/package.json"), json).unwrap()
  }

  #[test]
  fn test_resolve_bin_entry_value() {
    // should resolve the specified value
    let pkg_json = build_package_json(json!({
      "name": "pkg",
      "version": "1.1.1",
      "bin": {
        "bin1": "./value1",
        "bin2": "./value2",
        "pkg": "./value3",
      }
    }));
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, Some("bin1")).unwrap(),
      "./value1"
    );

    // should resolve the value with the same name when not specified
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, None).unwrap(),
      "./value3"
    );

    // should not resolve when specified value does not exist
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, Some("other"),)
        .err()
        .unwrap()
        .to_string(),
      concat!(
        "'/package.json' did not have a bin entry for 'other'\n",
        "\n",
        "Possibilities:\n",
        " * npm:pkg@1.1.1/bin1\n",
        " * npm:pkg@1.1.1/bin2\n",
        " * npm:pkg@1.1.1/pkg"
      )
    );

    // should not resolve when default value can't be determined
    let pkg_json = build_package_json(json!({
      "name": "pkg",
      "version": "1.1.1",
      "bin": {
        "bin": "./value1",
        "bin2": "./value2",
      }
    }));
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, None)
        .err()
        .unwrap()
        .to_string(),
      concat!(
        "'/package.json' did not have a bin entry for 'pkg'\n",
        "\n",
        "Possibilities:\n",
        " * npm:pkg@1.1.1/bin\n",
        " * npm:pkg@1.1.1/bin2",
      )
    );

    // should resolve since all the values are the same
    let pkg_json = build_package_json(json!({
      "name": "pkg",
      "version": "1.2.3",
      "bin": {
        "bin1": "./value",
        "bin2": "./value",
      }
    }));
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, None,).unwrap(),
      "./value"
    );

    // should not resolve when specified and is a string
    let pkg_json = build_package_json(json!({
      "name": "pkg",
      "version": "1.2.3",
      "bin": "./value",
    }));
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, Some("path"),)
        .err()
        .unwrap()
        .to_string(),
      "'/package.json' did not have a bin entry for 'path'"
    );

    // no version in the package.json
    let pkg_json = build_package_json(json!({
      "name": "pkg",
      "bin": {
        "bin1": "./value1",
        "bin2": "./value2",
      }
    }));
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, None)
        .err()
        .unwrap()
        .to_string(),
      concat!(
        "'/package.json' did not have a bin entry for 'pkg'\n",
        "\n",
        "Possibilities:\n",
        " * npm:pkg/bin1\n",
        " * npm:pkg/bin2",
      )
    );

    // no name or version in the package.json
    let pkg_json = build_package_json(json!({
      "bin": {
        "bin1": "./value1",
        "bin2": "./value2",
      }
    }));
    assert_eq!(
      resolve_bin_entry_value(&pkg_json, None)
        .err()
        .unwrap()
        .to_string(),
      concat!(
        "'/package.json' did not have a bin entry\n",
        "\n",
        "Possibilities:\n",
        " * bin1\n",
        " * bin2",
      )
    );
  }

  #[test]
  fn test_parse_package_name() {
    let dummy_referrer = Url::parse("http://example.com").unwrap();
    let dummy_referrer = UrlOrPathRef::from_url(&dummy_referrer);

    assert_eq!(
      parse_npm_pkg_name("fetch-blob", &dummy_referrer).unwrap(),
      ("fetch-blob", Cow::Borrowed("."), false)
    );
    assert_eq!(
      parse_npm_pkg_name("@vue/plugin-vue", &dummy_referrer).unwrap(),
      ("@vue/plugin-vue", Cow::Borrowed("."), true)
    );
    assert_eq!(
      parse_npm_pkg_name("@astrojs/prism/dist/highlighter", &dummy_referrer)
        .unwrap(),
      (
        "@astrojs/prism",
        Cow::Owned("./dist/highlighter".to_string()),
        true
      )
    );
  }

  #[test]
  fn test_with_known_extension() {
    let cases = &[
      ("test", "d.ts", "test.d.ts"),
      ("test.d.ts", "ts", "test.ts"),
      ("test.worker", "d.ts", "test.worker.d.ts"),
      ("test.d.mts", "js", "test.js"),
    ];
    for (path, ext, expected) in cases {
      let actual = with_known_extension(&PathBuf::from(path), ext);
      assert_eq!(actual.to_string_lossy(), *expected);
    }
  }

  #[test]
  fn test_types_package_name() {
    assert_eq!(types_package_name("name"), "@types/name");
    assert_eq!(
      types_package_name("@scoped/package"),
      "@types/scoped__package"
    );
  }

  #[test]
  fn test_types_versions() {
    let dir_path = PathBuf::from("/dir");
    let sys = InMemorySys::default();
    sys.fs_create_dir_all(dir_path.join("ts3.1")).unwrap();
    sys.fs_write(dir_path.join("file.d.ts"), "").unwrap();
    sys.fs_write(dir_path.join("ts3.1/file.d.ts"), "").unwrap();
    sys.fs_write(dir_path.join("ts3.1/file2.d.ts"), "").unwrap();
    let node_resolution_sys = NodeResolutionSys::new(sys, None);

    // asterisk key
    {
      let value = serde_json::json!({
        "*": ["ts3.1/*"]
      });
      let types_versions = TypesVersions {
        dir_path: &dir_path,
        value: value.as_object().unwrap(),
        sys: &node_resolution_sys,
      };
      assert_eq!(types_versions.map("file.d.ts").unwrap(), "ts3.1/file.d.ts");
      assert_eq!(
        types_versions.map("file2.d.ts").unwrap(),
        "ts3.1/file2.d.ts"
      );
      assert!(types_versions.map("non_existent/file.d.ts").is_none());
    }
    // specific file
    {
      let value = serde_json::json!({
        "types.d.ts": ["ts3.1/file.d.ts"]
      });
      let types_versions = TypesVersions {
        dir_path: &dir_path,
        value: value.as_object().unwrap(),
        sys: &node_resolution_sys,
      };
      assert_eq!(types_versions.map("types.d.ts").unwrap(), "ts3.1/file.d.ts");
      assert!(types_versions.map("file2.d.ts").is_none());
    }
    // multiple specific files
    {
      let value = serde_json::json!({
        "types.d.ts": ["ts3.1/file.d.ts"],
        "other.d.ts": ["ts3.1/file2.d.ts"],
      });
      let types_versions = TypesVersions {
        dir_path: &dir_path,
        value: value.as_object().unwrap(),
        sys: &node_resolution_sys,
      };
      assert_eq!(types_versions.map("types.d.ts").unwrap(), "ts3.1/file.d.ts");
      assert_eq!(
        types_versions.map("other.d.ts").unwrap(),
        "ts3.1/file2.d.ts"
      );
      assert!(types_versions.map("file2.d.ts").is_none());
    }
    // existing fallback
    {
      let value = serde_json::json!({
        "*": ["ts3.1/*", "ts3.1/file2.d.ts"]
      });
      let types_versions = TypesVersions {
        dir_path: &dir_path,
        value: value.as_object().unwrap(),
        sys: &node_resolution_sys,
      };
      assert_eq!(
        types_versions.map("testing/types.d.ts").unwrap(),
        "ts3.1/file2.d.ts"
      );
    }
    // text then asterisk in key
    {
      let value = serde_json::json!({
        "sub/*": ["ts3.1/file.d.ts"]
      });
      let types_versions = TypesVersions {
        dir_path: &dir_path,
        value: value.as_object().unwrap(),
        sys: &node_resolution_sys,
      };
      assert_eq!(
        types_versions.map("sub/types.d.ts").unwrap(),
        "ts3.1/file.d.ts"
      );
    }
    // text then asterisk in key and asterisk in value
    {
      let value = serde_json::json!({
        "sub/*": ["ts3.1/*"]
      });
      let types_versions = TypesVersions {
        dir_path: &dir_path,
        value: value.as_object().unwrap(),
        sys: &node_resolution_sys,
      };
      assert_eq!(
        types_versions.map("sub/file.d.ts").unwrap(),
        "ts3.1/file.d.ts"
      );
    }
  }
}
