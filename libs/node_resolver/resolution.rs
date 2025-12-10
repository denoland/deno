// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Error as AnyError;
use anyhow::bail;
use deno_media_type::MediaType;
use deno_package_json::PackageJson;
use deno_package_json::PackageJsonRc;
use deno_path_util::url_to_file_path;
use deno_semver::Version;
use deno_semver::VersionReq;
use lazy_regex::Lazy;
use regex::Regex;
use serde_json::Map;
use serde_json::Value;
use sys_traits::FileType;
use sys_traits::FsCanonicalize;
use sys_traits::FsDirEntry;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use sys_traits::OpenOptions;
use url::Url;

use crate::InNpmPackageChecker;
use crate::IsBuiltInNodeModuleChecker;
use crate::NpmPackageFolderResolver;
use crate::PackageJsonResolverRc;
use crate::PathClean;
use crate::cache::NodeResolutionSys;
use crate::errors;
use crate::errors::DataUrlReferrerError;
use crate::errors::FinalizeResolutionError;
use crate::errors::InvalidModuleSpecifierError;
use crate::errors::InvalidPackageTargetError;
use crate::errors::LegacyResolveError;
use crate::errors::MissingPkgJsonError;
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
use crate::errors::PackageSubpathFromDenoModuleResolveError;
use crate::errors::PackageSubpathResolveError;
use crate::errors::PackageSubpathResolveErrorKind;
use crate::errors::PackageTargetNotFoundError;
use crate::errors::PackageTargetResolveError;
use crate::errors::PackageTargetResolveErrorKind;
use crate::errors::ResolvePkgJsonBinExportError;
use crate::errors::ResolvePkgNpmBinaryCommandsError;
use crate::errors::TypesNotFoundError;
use crate::errors::TypesNotFoundErrorData;
use crate::errors::UnknownBuiltInNodeModuleError;
use crate::errors::UnsupportedDirImportError;
use crate::errors::UnsupportedEsmUrlSchemeError;
use crate::path::UrlOrPath;
use crate::path::UrlOrPathRef;

pub static IMPORT_CONDITIONS: &[Cow<'static, str>] = &[
  Cow::Borrowed("deno"),
  Cow::Borrowed("node"),
  Cow::Borrowed("import"),
];
pub static REQUIRE_CONDITIONS: &[Cow<'static, str>] =
  &[Cow::Borrowed("require"), Cow::Borrowed("node")];
static TYPES_ONLY_CONDITIONS: &[Cow<'static, str>] = &[Cow::Borrowed("types")];

#[derive(Debug, Default, Clone)]
pub struct NodeConditionOptions {
  pub conditions: Vec<Cow<'static, str>>,
  /// Provide a value to override the default import conditions.
  ///
  /// Defaults to `["deno", "node", "import"]`
  pub import_conditions_override: Option<Vec<Cow<'static, str>>>,
  /// Provide a value to override the default require conditions.
  ///
  /// Defaults to `["require", "node"]`
  pub require_conditions_override: Option<Vec<Cow<'static, str>>>,
}

#[derive(Debug, Clone)]
struct ConditionResolver {
  import_conditions: Cow<'static, [Cow<'static, str>]>,
  require_conditions: Cow<'static, [Cow<'static, str>]>,
}

impl ConditionResolver {
  pub fn new(options: NodeConditionOptions) -> Self {
    fn combine_conditions(
      user_conditions: Cow<'_, [Cow<'static, str>]>,
      override_default: Option<Vec<Cow<'static, str>>>,
      default_conditions: &'static [Cow<'static, str>],
    ) -> Cow<'static, [Cow<'static, str>]> {
      let default_conditions = override_default
        .map(Cow::Owned)
        .unwrap_or(Cow::Borrowed(default_conditions));
      if user_conditions.is_empty() {
        default_conditions
      } else {
        let mut new =
          Vec::with_capacity(user_conditions.len() + default_conditions.len());
        let mut append =
          |conditions: Cow<'_, [Cow<'static, str>]>| match conditions {
            Cow::Borrowed(conditions) => new.extend(conditions.iter().cloned()),
            Cow::Owned(conditions) => new.extend(conditions),
          };
        append(user_conditions);
        append(default_conditions);
        Cow::Owned(new)
      }
    }

    Self {
      import_conditions: combine_conditions(
        Cow::Borrowed(&options.conditions),
        options.import_conditions_override,
        IMPORT_CONDITIONS,
      ),
      require_conditions: combine_conditions(
        Cow::Owned(options.conditions),
        options.require_conditions_override,
        REQUIRE_CONDITIONS,
      ),
    }
  }

  pub fn resolve(
    &self,
    resolution_mode: ResolutionMode,
  ) -> &[Cow<'static, str>] {
    match resolution_mode {
      ResolutionMode::Import => &self.import_conditions,
      ResolutionMode::Require => &self.require_conditions,
    }
  }

  pub fn require_conditions(&self) -> &[Cow<'static, str>] {
    &self.require_conditions
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResolutionMode {
  Import,
  Require,
}

impl ResolutionMode {
  pub fn default_conditions(&self) -> &'static [Cow<'static, str>] {
    match self {
      ResolutionMode::Import => IMPORT_CONDITIONS,
      ResolutionMode::Require => REQUIRE_CONDITIONS,
    }
  }

  #[cfg(feature = "graph")]
  pub fn from_deno_graph(mode: deno_graph::source::ResolutionMode) -> Self {
    use deno_graph::source::ResolutionMode::*;
    match mode {
      Import => Self::Import,
      Require => Self::Require,
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

  #[cfg(feature = "graph")]
  pub fn from_deno_graph(kind: deno_graph::source::ResolutionKind) -> Self {
    use deno_graph::source::ResolutionKind::*;
    match kind {
      Execution => Self::Execution,
      Types => Self::Types,
    }
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

/// Kind of method that resolution succeeded with.
enum ResolvedMethod {
  Url,
  RelativeOrAbsolute,
  PackageImports,
  PackageExports,
  PackageSubPath,
}

#[derive(Debug, Default, Clone)]
pub struct NodeResolverOptions {
  pub conditions: NodeConditionOptions,
  pub is_browser_platform: bool,
  pub bundle_mode: bool,
  /// TypeScript version to use for typesVersions resolution and
  /// `types@req` exports resolution.
  pub typescript_version: Option<Version>,
}

#[derive(Debug)]
struct ResolutionConfig {
  pub bundle_mode: bool,
  pub prefer_browser_field: bool,
  pub typescript_version: Option<Version>,
}

#[sys_traits::auto_impl]
pub trait NodeResolverSys:
  FsCanonicalize + FsMetadata + FsRead + FsReadDir + FsOpen
{
}

#[allow(clippy::disallowed_types)]
pub type NodeResolverRc<
  TInNpmPackageChecker,
  TIsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver,
  TSys,
> = deno_maybe_sync::MaybeArc<
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
  TSys: NodeResolverSys,
> {
  in_npm_pkg_checker: TInNpmPackageChecker,
  is_built_in_node_module_checker: TIsBuiltInNodeModuleChecker,
  npm_pkg_folder_resolver: TNpmPackageFolderResolver,
  pkg_json_resolver: PackageJsonResolverRc<TSys>,
  sys: NodeResolutionSys<TSys>,
  condition_resolver: ConditionResolver,
  resolution_config: ResolutionConfig,
}

impl<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: NodeResolverSys,
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
      condition_resolver: ConditionResolver::new(NodeConditionOptions {
        conditions: options.conditions.conditions,
        import_conditions_override: options
          .conditions
          .import_conditions_override
          .or_else(|| {
            if options.is_browser_platform {
              Some(vec![Cow::Borrowed("browser"), Cow::Borrowed("import")])
            } else {
              None
            }
          }),
        require_conditions_override: options
          .conditions
          .require_conditions_override
          .or_else(|| {
            if options.is_browser_platform {
              Some(vec![Cow::Borrowed("browser"), Cow::Borrowed("require")])
            } else {
              None
            }
          }),
      }),
      resolution_config: ResolutionConfig {
        bundle_mode: options.bundle_mode,
        prefer_browser_field: options.is_browser_platform,
        typescript_version: options.typescript_version,
      },
    }
  }

  pub fn require_conditions(&self) -> &[Cow<'static, str>] {
    self.condition_resolver.require_conditions()
  }

  pub fn in_npm_package(&self, specifier: &Url) -> bool {
    self.in_npm_pkg_checker.in_npm_package(specifier)
  }

  #[inline(always)]
  pub fn is_builtin_node_module(&self, specifier: &str) -> bool {
    self
      .is_built_in_node_module_checker
      .is_builtin_node_module(specifier)
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

    if self.is_builtin_node_module(specifier) {
      return Ok(NodeResolution::BuiltIn(specifier.to_string()));
    }

    if let Ok(url) = Url::parse(specifier) {
      if url.scheme() == "data" {
        return Ok(NodeResolution::Module(UrlOrPath::Url(url)));
      }

      if let Some(module_name) =
        self.get_module_name_from_builtin_node_module_url(&url)?
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

    let conditions = self.condition_resolver.resolve(resolution_mode);
    let referrer = UrlOrPathRef::from_url(referrer);
    let (url, resolved_kind) = self.module_resolve(
      specifier,
      &referrer,
      resolution_mode,
      conditions,
      resolution_kind,
    )?;

    let url_or_path = self.finalize_resolution(
      url,
      resolved_kind,
      resolution_mode,
      conditions,
      resolution_kind,
      Some(&referrer),
    )?;
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
    conditions: &[Cow<'static, str>],
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
        .map_err(PackageImportsResolveErrorKind::PkgJsonLoad)
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
    resolution_mode: ResolutionMode,
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
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
    let path = match p_str.strip_suffix('/') {
      Some(s) => Cow::Borrowed(Path::new(s)),
      None => Cow::Owned(path),
    };

    let maybe_file_type = self.sys.get_file_type(&path);
    match maybe_file_type {
      Ok(FileType::Dir) => {
        if resolution_mode == ResolutionMode::Import
          && !self.resolution_config.bundle_mode
        {
          let suggestion = self.directory_import_suggestion(&path);
          Err(
            UnsupportedDirImportError {
              dir_url: UrlOrPath::Path(path.into_owned()),
              maybe_referrer: maybe_referrer.map(|r| r.display()),
              suggestion,
            }
            .into(),
          )
        } else {
          // prefer the file over the directory
          let path_with_ext = with_known_extension(&path, "js");
          if self.sys.is_file(&path_with_ext) {
            Ok(UrlOrPath::Path(path_with_ext))
          } else {
            let (resolved_url, resolved_method) = self
              .resolve_package_dir_subpath(
                &path,
                ".",
                maybe_referrer,
                resolution_mode,
                conditions,
                resolution_kind,
              )?;
            self.finalize_resolution(
              resolved_url,
              resolved_method,
              resolution_mode,
              conditions,
              resolution_kind,
              maybe_referrer,
            )
          }
        }
      }
      Ok(FileType::File) => {
        // prefer returning the url to avoid re-allocating in the CLI crate
        Ok(
          maybe_url
            .map(UrlOrPath::Url)
            .unwrap_or(UrlOrPath::Path(path.into_owned())),
        )
      }
      _ => {
        if let Err(e) = maybe_file_type
          && (resolution_mode == ResolutionMode::Require
            || self.resolution_config.bundle_mode)
          && e.kind() == std::io::ErrorKind::NotFound
        {
          let file_with_ext = with_known_extension(&path, "js");
          if self.sys.is_file(&file_with_ext) {
            return Ok(UrlOrPath::Path(file_with_ext));
          }
        }

        Err(
          ModuleNotFoundError {
            suggested_ext: self
              .module_not_found_ext_suggestion(&path, resolved_method),
            specifier: UrlOrPath::Path(path.into_owned()),
            maybe_referrer: maybe_referrer.map(|r| r.display()),
          }
          .into(),
        )
      }
    }
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

  fn directory_import_suggestion(
    &self,
    dir_import_path: &Path,
  ) -> Option<String> {
    let dir_index_paths = ["index.mjs", "index.js", "index.cjs"]
      .into_iter()
      .map(|file_name| dir_import_path.join(file_name));
    let file_paths = [
      with_known_extension(dir_import_path, "js"),
      with_known_extension(dir_import_path, "mjs"),
      with_known_extension(dir_import_path, "cjs"),
    ];
    dir_index_paths
      .chain(file_paths)
      .chain(
        std::iter::once_with(|| {
          // check if this directory has a package.json
          let package_json_path = dir_import_path.join("package.json");
          let pkg_json = self
            .pkg_json_resolver
            .load_package_json(&package_json_path)
            .ok()??;
          let main = pkg_json.main.as_ref()?;
          Some(dir_import_path.join(main))
        })
        .flatten(),
      )
      .map(|p| deno_path_util::normalize_path(Cow::Owned(p)))
      .find(|p| self.sys.is_file(p))
      .and_then(|suggested_file_path| {
        let pkg_json = self
          .pkg_json_resolver
          .get_closest_package_jsons(&suggested_file_path)
          .filter_map(|pkg_json| pkg_json.ok())
          .find(|p| p.name.is_some())?;
        let pkg_name = pkg_json.name.as_ref()?;
        let sub_path = suggested_file_path
          .strip_prefix(pkg_json.dir_path())
          .ok()?
          .to_string_lossy()
          .replace("\\", "/");
        Some(format!("{}/{}", pkg_name, sub_path))
      })
  }

  pub fn resolve_package_subpath_from_deno_module(
    &self,
    package_dir: &Path,
    package_subpath: Option<&str>,
    maybe_referrer: Option<&Url>,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<UrlOrPath, PackageSubpathFromDenoModuleResolveError> {
    // todo(dsherret): don't allocate a string here (maybe use an
    // enum that says the subpath is not prefixed with a ./)
    let package_subpath = package_subpath
      .map(|s| Cow::Owned(format!("./{s}")))
      .unwrap_or_else(|| Cow::Borrowed("."));
    let maybe_referrer = maybe_referrer.map(UrlOrPathRef::from_url);
    let conditions = self.condition_resolver.resolve(resolution_mode);
    let (resolved_url, resolved_method) = self.resolve_package_dir_subpath(
      package_dir,
      &package_subpath,
      maybe_referrer.as_ref(),
      resolution_mode,
      conditions,
      resolution_kind,
    )?;
    let url_or_path = self.finalize_resolution(
      resolved_url,
      resolved_method,
      resolution_mode,
      conditions,
      resolution_kind,
      maybe_referrer.as_ref(),
    )?;
    Ok(url_or_path)
  }

  pub fn resolve_binary_export(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
  ) -> Result<BinValue, ResolvePkgJsonBinExportError> {
    let (pkg_json, items) = self
      .resolve_npm_binary_commands_for_package_with_pkg_json(package_folder)?;
    let path =
      resolve_bin_entry_value(&pkg_json, &items, sub_path).map_err(|err| {
        ResolvePkgJsonBinExportError::InvalidBinProperty {
          message: err.to_string(),
        }
      })?;
    Ok(path.clone())
  }

  pub fn resolve_npm_binary_commands_for_package(
    &self,
    package_folder: &Path,
  ) -> Result<BTreeMap<String, BinValue>, ResolvePkgNpmBinaryCommandsError> {
    let (_pkg_json, items) = self
      .resolve_npm_binary_commands_for_package_with_pkg_json(package_folder)?;
    Ok(items)
  }

  fn resolve_npm_binary_commands_for_package_with_pkg_json(
    &self,
    package_folder: &Path,
  ) -> Result<
    (PackageJsonRc, BTreeMap<String, BinValue>),
    ResolvePkgNpmBinaryCommandsError,
  > {
    let pkg_json_path = package_folder.join("package.json");
    let Some(package_json) =
      self.pkg_json_resolver.load_package_json(&pkg_json_path)?
    else {
      return Err(ResolvePkgNpmBinaryCommandsError::MissingPkgJson(
        MissingPkgJsonError { pkg_json_path },
      ));
    };
    let bins = package_json.resolve_bins()?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    let items = match bins {
      deno_package_json::PackageJsonBins::Directory(path_buf) => {
        self.resolve_npm_commands_from_bin_dir(&path_buf)
      }
      deno_package_json::PackageJsonBins::Bins(items) => items
        .into_iter()
        .filter_map(|(command, path)| {
          let bin_value = bin_value_from_file(&path, &self.sys)?;
          Some((command, bin_value))
        })
        .collect(),
    };
    Ok((package_json, items))
  }

  pub fn resolve_npm_commands_from_bin_dir(
    &self,
    bin_dir: &Path,
  ) -> BTreeMap<String, BinValue> {
    log::debug!("Resolving npm commands in '{}'.", bin_dir.display());
    let mut result = BTreeMap::new();
    match self.sys.fs_read_dir(bin_dir) {
      Ok(entries) => {
        for entry in entries {
          let Ok(entry) = entry else {
            continue;
          };
          if let Some((command, bin_value)) =
            self.resolve_bin_dir_entry_command(entry)
          {
            result.insert(command, bin_value);
          }
        }
      }
      Err(err) => {
        log::debug!("Failed read_dir for '{}': {:#}", bin_dir.display(), err);
      }
    }
    result
  }

  fn resolve_bin_dir_entry_command(
    &self,
    entry: TSys::ReadDirEntry,
  ) -> Option<(String, BinValue)> {
    if entry.path().extension().is_some() {
      return None; // only look at files without extensions (even on Windows)
    }
    let file_type = entry.file_type().ok()?;
    let path = if file_type.is_file() {
      entry.path()
    } else if file_type.is_symlink() {
      Cow::Owned(self.sys.fs_canonicalize(entry.path()).ok()?)
    } else {
      return None;
    };
    let command_name = entry.file_name().to_string_lossy().into_owned();
    let bin_value = bin_value_from_file(&path, &self.sys)?;
    Some((command_name, bin_value))
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
    conditions: &[Cow<'static, str>],
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
    conditions: &[Cow<'static, str>],
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
      if let Some(specific_dts_path) = specific_dts_path
        && sys.exists_(&specific_dts_path)
      {
        return Some(specific_dts_path);
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
  pub fn resolve_package_import(
    &self,
    name: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    referrer_pkg_json: Option<&PackageJson>,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<UrlOrPath, PackageImportsResolveError> {
    self
      .package_imports_resolve_internal(
        name,
        maybe_referrer,
        resolution_mode,
        referrer_pkg_json,
        self.condition_resolver.resolve(resolution_mode),
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
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, PackageImportsResolveError> {
    if name == "#" || name.ends_with('/') {
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

    if let Some(pkg_json) = &referrer_pkg_json
      && let Some(resolved_import) = resolve_pkg_json_import(pkg_json, name)
    {
      let maybe_resolved = self.resolve_package_target(
        &pkg_json.path,
        resolved_import.target,
        resolved_import.sub_path,
        resolved_import.package_sub_path,
        maybe_referrer,
        resolution_mode,
        resolved_import.is_pattern,
        true,
        conditions,
        resolution_kind,
      )?;
      if let Some(resolved) = maybe_resolved {
        return Ok(resolved);
      }
    }

    Err(
      PackageImportNotDefinedError {
        name: name.to_string(),
        package_json_path: referrer_pkg_json.map(|p| p.path.clone()),
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
    conditions: &[Cow<'static, str>],
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
            if self
              .get_module_name_from_builtin_node_module_url(&url)?
              .is_some()
            {
              return Ok(MaybeTypesResolvedUrl(LocalUrlOrPath::Url(url)));
            }
          }
          Err(_) => {
            let export_target = if pattern {
              pattern_re.replace(target, |_caps: &regex::Captures| subpath)
            } else if subpath.is_empty() {
              Cow::Borrowed(target)
            } else {
              Cow::Owned(format!("{target}{subpath}"))
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
                | NodeJsErrorCode::ERR_UNKNOWN_BUILTIN_MODULE
                | NodeJsErrorCode::ERR_TYPES_NOT_FOUND => {
                  Err(PackageTargetResolveErrorKind::PackageResolve(err).into())
                }
                NodeJsErrorCode::ERR_MODULE_NOT_FOUND => Err(
                  PackageTargetResolveErrorKind::NotFound(
                    PackageTargetNotFoundError {
                      pkg_json_path: package_json_path.to_path_buf(),
                      target: export_target.to_string(),
                      maybe_resolved: err
                        .maybe_specifier()
                        .map(|c| c.into_owned()),
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
    conditions: &[Cow<'static, str>],
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
    conditions: &[Cow<'static, str>],
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
          || conditions.contains(&Cow::Borrowed(key))
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
    }

    Ok(None)
  }

  fn matches_types_key(&self, key: &str) -> bool {
    if key == "types" {
      return true;
    }
    let Some(ts_version) = &self.resolution_config.typescript_version else {
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
    conditions: &[Cow<'static, str>],
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
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, PackageExportsResolveError> {
    if let Some(target) = package_exports.get(package_subpath)
      && package_subpath.find('*').is_none()
      && !package_subpath.ends_with('/')
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
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageResolveError> {
    let (package_name, package_subpath, _is_scoped) =
      parse_npm_pkg_name(specifier, referrer)?;

    if let Some(package_config) = self
      .pkg_json_resolver
      .get_closest_package_json(referrer.path()?)?
    {
      // ResolveSelf
      if package_config.name.as_deref() == Some(package_name)
        && let Some(exports) = &package_config.exports
      {
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
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageResolveError> {
    let resolve = |package_dir: &Path| {
      self.resolve_package_dir_subpath(
        package_dir,
        package_subpath,
        Some(referrer),
        resolution_mode,
        conditions,
        resolution_kind,
      )
    };
    let result: Result<_, PackageResolveError> = self
      .npm_pkg_folder_resolver
      .resolve_package_folder_from_package(package_name, referrer)
      .map_err(|err| err.into())
      .and_then(|package_dir| resolve(&package_dir).map_err(|e| e.into()));

    if resolution_kind.is_types() && result.is_err() {
      // try to resolve with the @types package based on the package name
      let maybe_types_package_dir = self
        .resolve_types_package_folder_with_name_and_version(
          package_name,
          None,
          Some(referrer),
        );
      if let Some(types_package_dir) = maybe_types_package_dir
        && let Ok(result) = resolve(&types_package_dir)
      {
        return Ok(result);
      }
    }
    result
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_dir_subpath(
    &self,
    package_dir_path: &Path,
    package_subpath: &str,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
  ) -> Result<(MaybeTypesResolvedUrl, ResolvedMethod), PackageSubpathResolveError>
  {
    let package_json_path = package_dir_path.join("package.json");
    match self
      .pkg_json_resolver
      .load_package_json(&package_json_path)?
    {
      Some(pkg_json) => {
        let result = self.resolve_package_subpath(
          &pkg_json,
          package_subpath,
          maybe_referrer,
          resolution_mode,
          conditions,
          resolution_kind,
        );
        if resolution_kind.is_types()
          && result.is_err()
          && let Some(types_pkg_dir) = self
            .resolve_types_package_folder_from_package_json(
              &pkg_json,
              maybe_referrer,
            )
          && let Ok(result) = self.resolve_package_dir_subpath(
            &types_pkg_dir,
            package_subpath,
            maybe_referrer,
            resolution_mode,
            conditions,
            resolution_kind,
          )
        {
          Ok(result)
        } else {
          result
        }
      }
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
    conditions: &[Cow<'static, str>],
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
        let ts_version = self.resolution_config.typescript_version.as_ref()?;
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
    conditions: &[Cow<'static, str>],
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
    conditions: &[Cow<'static, str>],
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

  pub(crate) fn legacy_fallback_resolve<'a>(
    &self,
    package_json: &'a PackageJson,
  ) -> Option<&'a str> {
    fn filter_empty(value: Option<&str>) -> Option<&str> {
      value.map(|v| v.trim()).filter(|v| !v.is_empty())
    }
    if self.resolution_config.bundle_mode {
      let maybe_browser = if self.resolution_config.prefer_browser_field {
        filter_empty(package_json.browser.as_deref())
      } else {
        None
      };
      maybe_browser
        .or(filter_empty(package_json.module.as_deref()))
        .or(filter_empty(package_json.main.as_deref()))
    } else {
      filter_empty(package_json.main.as_deref())
    }
  }

  fn legacy_main_resolve(
    &self,
    package_json: &PackageJson,
    maybe_referrer: Option<&UrlOrPathRef>,
    resolution_mode: ResolutionMode,
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
  ) -> Result<MaybeTypesResolvedUrl, LegacyResolveError> {
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
          if let Some(main) = self.legacy_fallback_resolve(package_json) {
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
      self
        .legacy_fallback_resolve(package_json)
        .map(Cow::Borrowed)
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
      // Node only resolves index.js and not index.cjs/mjs
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
          maybe_referrer: maybe_referrer.map(|r| r.display()),
          suggested_ext: None,
        }
        .into(),
      )
    }
  }

  fn resolve_types_package_folder_from_package_json(
    &self,
    pkg_json: &PackageJson,
    maybe_referrer: Option<&UrlOrPathRef>,
  ) -> Option<PathBuf> {
    let package_name = pkg_json.name.as_deref()?;
    let maybe_version = pkg_json
      .version
      .as_ref()
      .and_then(|v| Version::parse_from_npm(v).ok());
    self.resolve_types_package_folder_with_name_and_version(
      package_name,
      maybe_version.as_ref(),
      maybe_referrer,
    )
  }

  fn resolve_types_package_folder_with_name_and_version(
    &self,
    package_name: &str,
    maybe_version: Option<&Version>,
    maybe_referrer: Option<&UrlOrPathRef>,
  ) -> Option<PathBuf> {
    let types_package_name = types_package_name(package_name)?;
    log::debug!(
      "Attempting to resolve types package '{}@{}'.",
      types_package_name,
      maybe_version
        .as_ref()
        .map(|s| s.to_string())
        .as_deref()
        .unwrap_or("*")
    );
    self.npm_pkg_folder_resolver.resolve_types_package_folder(
      &types_package_name,
      maybe_version,
      maybe_referrer,
    )
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
      // so canonicalize then check if it's in the node_modules directory.
      let specifier = resolve_specifier_into_node_modules(&self.sys, specifier);
      return Some(specifier);
    }

    None
  }

  /// Ex. returns `fs` for `node:fs`
  fn get_module_name_from_builtin_node_module_url<'url>(
    &self,
    url: &'url Url,
  ) -> Result<Option<&'url str>, UnknownBuiltInNodeModuleError> {
    if url.scheme() != "node" {
      return Ok(None);
    }

    let module_name = url.path();

    if !self
      .is_built_in_node_module_checker
      .is_builtin_node_module(module_name)
    {
      return Err(UnknownBuiltInNodeModuleError {
        module_name: module_name.to_string(),
      });
    }
    Ok(Some(module_name))
  }
}

struct ResolvedPkgJsonImport<'a> {
  pub target: &'a serde_json::Value,
  pub sub_path: &'a str,
  pub package_sub_path: &'a str,
  pub is_pattern: bool,
}

fn resolve_pkg_json_import<'a>(
  pkg_json: &'a PackageJson,
  name: &'a str,
) -> Option<ResolvedPkgJsonImport<'a>> {
  let imports = pkg_json.imports.as_ref()?;
  if let Some((name, target)) =
    imports.get_key_value(name).filter(|_| !name.contains('*'))
  {
    Some(ResolvedPkgJsonImport {
      target,
      sub_path: "",
      package_sub_path: name,
      is_pattern: false,
    })
  } else {
    let mut best_match: &'a str = "";
    let mut best_match_subpath: &'a str = "";
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
            best_match_subpath =
              &name[pattern_index..(name.len() - pattern_trailer.len())];
          }
        }
      }
    }

    if !best_match.is_empty() {
      let target = imports.get(best_match).unwrap();
      Some(ResolvedPkgJsonImport {
        target,
        sub_path: best_match_subpath,
        package_sub_path: best_match,
        is_pattern: true,
      })
    } else {
      None
    }
  }
}

fn bin_value_from_file<TSys: FsOpen>(
  path: &Path,
  sys: &NodeResolutionSys<TSys>,
) -> Option<BinValue> {
  let mut file = match sys.fs_open(path, OpenOptions::new().read()) {
    Ok(file) => file,
    Err(err) => {
      if err.kind() == std::io::ErrorKind::NotFound {
        return None;
      }
      log::debug!(
        "Failed to open bin file '{}': {:#}; treating as executable",
        path.display(),
        err,
      );
      return Some(BinValue::Executable(path.to_path_buf()));
    }
  };
  let mut buf = [0; 4];
  let (is_binary, buf): (bool, &[u8]) = {
    let result = file.read_exact(&mut buf);
    if let Err(err) = result {
      log::debug!("Failed to read binary file '{}': {:#}", path.display(), err);
      // safer fallback to assume it's a binary
      (true, &[])
    } else {
      (
        is_binary(&buf) || (!(buf[0] == b'#' && buf[1] == b'!')),
        &buf[..],
      )
    }
  };

  if is_binary {
    return Some(BinValue::Executable(path.to_path_buf()));
  }
  let mut buf_read = BufReader::new(file);
  let mut contents = Vec::new();
  contents.extend_from_slice(buf);
  if let Ok(len) = buf_read.read_to_end(&mut contents)
    && len > 0
    && let Ok(contents) = String::from_utf8(contents)
    && let Some(path) =
      resolve_execution_path_from_npx_shim(Cow::Borrowed(path), &contents)
  {
    return Some(BinValue::JsFile(path));
  }

  Some(BinValue::Executable(path.to_path_buf()))
}

/// This is not ideal, but it works ok because it allows us to bypass
/// the shebang and execute the script directly with Deno.
fn resolve_execution_path_from_npx_shim(
  file_path: Cow<Path>,
  text: &str,
) -> Option<PathBuf> {
  static SCRIPT_PATH_RE: Lazy<Regex> =
    lazy_regex::lazy_regex!(r#"exec\s+node\s+"\$basedir\/([^"]+)" "\$@""#);

  let maybe_first_line = {
    let index = text.find("\n")?;
    Some(&text[0..index])
  };

  if let Some(first_line) = maybe_first_line {
    // NOTE(bartlomieju): this is not perfect, but handle two most common scenarios
    // where Node is run without any args.
    if first_line == "#!/usr/bin/env node"
      || first_line == "#!/usr/bin/env -S node"
    {
      // launch this file itself because it's a JS file
      return Some(file_path.into_owned());
    }
  }

  // Search for...
  // > "$basedir/../next/dist/bin/next" "$@"
  // ...which is what it will look like on Windows
  SCRIPT_PATH_RE
    .captures(text)
    .and_then(|c| c.get(1))
    .map(|relative_path| {
      file_path.parent().unwrap().join(relative_path.as_str())
    })
}

fn resolve_bin_entry_value<'a>(
  package_json: &PackageJson,
  bins: &'a BTreeMap<String, BinValue>,
  bin_name: Option<&str>,
) -> Result<&'a BinValue, AnyError> {
  if bins.is_empty() {
    bail!(
      "'{}' did not have a bin property with a string or non-empty object value",
      package_json.path.display()
    );
  }
  let default_bin = package_json.resolve_default_bin_name().ok();
  let searching_bin = bin_name.or(default_bin);
  match searching_bin.and_then(|bin_name| bins.get(bin_name)) {
    Some(bin) => Ok(bin),
    _ => {
      if bins.len() > 1
        && let Some(first) = bins.values().next()
        && bins.values().all(|bin| bin == first)
      {
        return Ok(first);
      }
      if bin_name.is_none()
        && bins.len() == 1
        && let Some(first) = bins.values().next()
      {
        return Ok(first);
      }
      let default_bin = package_json.resolve_default_bin_name().ok();
      let prefix = package_json
        .name
        .as_ref()
        .map(|n| {
          let mut prefix = format!("npm:{}", n);
          if let Some(version) = &package_json.version {
            prefix.push('@');
            prefix.push_str(version);
          }
          prefix
        })
        .unwrap_or_default();
      let keys = bins
        .keys()
        .map(|k| {
          if prefix.is_empty() {
            format!(" * {k}")
          } else if Some(k.as_str()) == default_bin {
            format!(" * {prefix}")
          } else {
            format!(" * {prefix}/{k}")
          }
        })
        .collect::<Vec<_>>();
      bail!(
        "'{}' did not have a bin entry for '{}'{}",
        package_json.path.display(),
        searching_bin.unwrap_or("<unspecified>"),
        if keys.is_empty() {
          "".to_string()
        } else {
          format!("\n\nPossibilities:\n{}", keys.join("\n"))
        }
      )
    }
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

/// Gets the corresponding @types package for the provided package name
/// returning `None` when the package is already a @types package.
fn types_package_name(package_name: &str) -> Option<String> {
  if package_name.starts_with("@types/") {
    return None;
  }
  // Scoped packages will get two underscores for each slash
  // https://github.com/DefinitelyTyped/DefinitelyTyped/tree/15f1ece08f7b498f4b9a2147c2a46e94416ca777#what-about-scoped-packages
  capacity_builder::StringBuilder::build(|builder| {
    builder.append("@types/");
    for (i, c) in package_name.chars().enumerate() {
      match c {
        '@' if i == 0 => {
          // ignore
        }
        '/' => {
          builder.append("__");
        }
        c => builder.append(c),
      }
    }
  })
  .ok()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinValue {
  JsFile(PathBuf),
  Executable(PathBuf),
}

impl BinValue {
  pub fn path(&self) -> &Path {
    match self {
      BinValue::JsFile(path) => path,
      BinValue::Executable(path) => path,
    }
  }
}
fn is_binary(data: &[u8]) -> bool {
  is_elf(data) || is_macho(data) || is_pe(data)
}

// vendored from libsui because they're super small
/// Check if the given data is an ELF64 binary
fn is_elf(data: &[u8]) -> bool {
  if data.len() < 4 {
    return false;
  }
  let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
  magic == 0x7f454c46
}

/// Check if the given data is a 64-bit Mach-O binary
fn is_macho(data: &[u8]) -> bool {
  if data.len() < 4 {
    return false;
  }
  let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
  magic == 0xfeedfacf
}

/// Check if the given data is a PE32+ binary
fn is_pe(data: &[u8]) -> bool {
  if data.len() < 2 {
    return false;
  }
  let magic = u16::from_le_bytes([data[0], data[1]]);
  magic == 0x5a4d
}

#[cfg(test)]
mod tests {
  use deno_package_json::PackageJsonBins;
  use serde_json::json;
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsWrite;
  use sys_traits::impls::InMemorySys;

  use super::*;

  fn build_package_json(json: Value) -> PackageJson {
    PackageJson::load_from_value(PathBuf::from("/package.json"), json).unwrap()
  }

  fn resolve_bins(package_json: &PackageJson) -> BTreeMap<String, BinValue> {
    match package_json.resolve_bins().unwrap() {
      PackageJsonBins::Directory(_) => unreachable!(),
      PackageJsonBins::Bins(bins) => bins
        .into_iter()
        .map(|(k, v)| (k, BinValue::JsFile(v)))
        .collect(),
    }
  }

  #[test]
  fn test_resolve_bin_entry_value() {
    // should resolve the specified value
    {
      let pkg_json = build_package_json(json!({
        "name": "pkg",
        "version": "1.1.1",
        "bin": {
          "bin1": "./value1",
          "bin2": "./value2",
          "pkg": "./value3",
        }
      }));
      let bins = resolve_bins(&pkg_json);
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("bin1"))
          .unwrap()
          .path(),
        pkg_json.dir_path().join("./value1")
      );
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("pkg"))
          .unwrap()
          .path(),
        pkg_json.dir_path().join("./value3")
      );

      // should not resolve when specified value does not exist
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("other"))
          .err()
          .unwrap()
          .to_string(),
        concat!(
          "'/package.json' did not have a bin entry for 'other'\n",
          "\n",
          "Possibilities:\n",
          " * npm:pkg@1.1.1/bin1\n",
          " * npm:pkg@1.1.1/bin2\n",
          " * npm:pkg@1.1.1"
        )
      );
    }

    // should not resolve when default value can't be determined
    {
      let pkg_json = build_package_json(json!({
        "name": "pkg",
        "version": "1.1.1",
        "bin": {
          "bin": "./value1",
          "bin2": "./value2",
        }
      }));
      let bins = resolve_bins(&pkg_json);
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("pkg"))
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
    }

    // should resolve since all the values are the same
    {
      let pkg_json = build_package_json(json!({
        "name": "pkg",
        "version": "1.2.3",
        "bin": {
          "bin1": "./value",
          "bin2": "./value",
        }
      }));
      let bins = resolve_bins(&pkg_json);
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("pkg"))
          .unwrap()
          .path(),
        pkg_json.dir_path().join("./value")
      );
    }

    // should resolve when not specified and only one value
    {
      let pkg_json = build_package_json(json!({
        "name": "pkg",
        "version": "1.2.3",
        "bin": {
          "something": "./value",
        }
      }));
      let bins = resolve_bins(&pkg_json);
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, None)
          .unwrap()
          .path(),
        pkg_json.dir_path().join("./value")
      );
    }

    // should not resolve when specified and is a string
    {
      let pkg_json = build_package_json(json!({
        "name": "pkg",
        "version": "1.2.3",
        "bin": "./value",
      }));
      let bins = resolve_bins(&pkg_json);
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("path"))
          .err()
          .unwrap()
          .to_string(),
        concat!(
          "'/package.json' did not have a bin entry for 'path'\n",
          "\n",
          "Possibilities:\n",
          " * npm:pkg@1.2.3"
        )
      );
    }

    // no version in the package.json
    {
      let pkg_json = build_package_json(json!({
        "name": "pkg",
        "bin": {
          "bin1": "./value1",
          "bin2": "./value2",
        }
      }));
      let bins = resolve_bins(&pkg_json);
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("pkg"))
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
    }

    // no name or version in the package.json
    {
      let pkg_json = build_package_json(json!({
        "bin": {
          "bin1": "./value1",
          "bin2": "./value2",
        }
      }));
      let bins = resolve_bins(&pkg_json);
      assert_eq!(
        resolve_bin_entry_value(&pkg_json, &bins, Some("bin"))
          .err()
          .unwrap()
          .to_string(),
        concat!(
          "'/package.json' did not have a bin entry for 'bin'\n",
          "\n",
          "Possibilities:\n",
          " * bin1\n",
          " * bin2",
        )
      );
    }
  }

  #[test]
  fn test_parse_npm_pkg_name() {
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
      let actual = with_known_extension(Path::new(path), ext);
      assert_eq!(actual.to_string_lossy(), *expected);
    }
  }

  #[test]
  fn test_types_package_name() {
    assert_eq!(types_package_name("name").unwrap(), "@types/name");
    assert_eq!(
      types_package_name("@scoped/package").unwrap(),
      "@types/scoped__package"
    );
    assert_eq!(types_package_name("@types/node"), None);
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

  #[test]
  fn test_resolve_execution_path_from_npx_shim() {
    // example shim on unix
    let unix_shim = r#"#!/usr/bin/env node
"use strict";
console.log('Hi!');
"#;
    let path = PathBuf::from("/node_modules/.bin/example");
    assert_eq!(
      resolve_execution_path_from_npx_shim(Cow::Borrowed(&path), unix_shim)
        .unwrap(),
      path
    );
    // example shim on unix
    let unix_shim = r#"#!/usr/bin/env -S node
"use strict";
console.log('Hi!');
"#;
    let path = PathBuf::from("/node_modules/.bin/example");
    assert_eq!(
      resolve_execution_path_from_npx_shim(Cow::Borrowed(&path), unix_shim)
        .unwrap(),
      path
    );
    // example shim on windows
    let windows_shim = r#"#!/bin/sh
basedir=$(dirname "$(echo "$0" | sed -e 's,\\,/,g')")

case `uname` in
    *CYGWIN*|*MINGW*|*MSYS*) basedir=`cygpath -w "$basedir"`;;
esac

if [ -x "$basedir/node" ]; then
  exec "$basedir/node"  "$basedir/../example/bin/example" "$@"
else
  exec node  "$basedir/../example/bin/example" "$@"
fi"#;
    assert_eq!(
      resolve_execution_path_from_npx_shim(Cow::Borrowed(&path), windows_shim)
        .unwrap(),
      path.parent().unwrap().join("../example/bin/example")
    );
  }
}
