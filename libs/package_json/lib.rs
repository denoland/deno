// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]
#![deny(clippy::unused_async)]
#![deny(clippy::unnecessary_wraps)]

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_error::JsError;
use deno_semver::StackString;
use deno_semver::VersionReq;
use deno_semver::npm::NpmVersionReqParseError;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use sys_traits::FsRead;
use thiserror::Error;
use url::Url;

#[allow(clippy::disallowed_types, reason = "arc wrapper type")]
pub type PackageJsonRc = deno_maybe_sync::MaybeArc<PackageJson>;
#[allow(clippy::disallowed_types, reason = "arc wrapper type")]
pub type PackageJsonDepsRc = deno_maybe_sync::MaybeArc<PackageJsonDeps>;
#[allow(clippy::disallowed_types, reason = "once lock wrapper type")]
type PackageJsonDepsRcCell = deno_maybe_sync::MaybeOnceLock<PackageJsonDepsRc>;

pub enum PackageJsonCacheResult {
  Hit(Option<PackageJsonRc>),
  NotCached,
}

pub trait PackageJsonCache {
  fn get(&self, path: &Path) -> PackageJsonCacheResult;
  fn set(&self, path: PathBuf, package_json: Option<PackageJsonRc>);
}

#[derive(Debug, Clone)]
pub enum PackageJsonBins {
  Directory(PathBuf),
  Bins(BTreeMap<String, PathBuf>),
}

/// The value of the `sideEffects` field in a `package.json`.
///
/// See https://webpack.js.org/guides/tree-shaking/#mark-the-file-as-side-effect-free
/// for details on how this field is interpreted by bundlers.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PackageJsonSideEffects {
  /// `false` means the package has no side effects;
  /// `true` (or omitted) means every file may have side effects.
  Bool(bool),
  /// A list of glob patterns matching files that have side effects.
  /// All other files in the package are treated as side-effect-free.
  Patterns(Vec<String>),
}

/// An entry in the object form of `package.json`'s `browser` field.
///
/// A string value remaps the key to a different specifier; `false` marks the
/// key as disabled (bundlers should substitute an empty module).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum BrowserMapEntry {
  Replace(String),
  Disabled,
}

#[derive(Debug, Clone, Error, JsError, PartialEq, Eq)]
#[class(generic)]
#[error("'{}' did not have a name", pkg_json_path.display())]
pub struct MissingPkgJsonNameError {
  pkg_json_path: PathBuf,
}

#[derive(Debug, Clone, JsError, PartialEq, Eq, Boxed)]
pub struct PackageJsonDepValueParseError(
  pub Box<PackageJsonDepValueParseErrorKind>,
);

#[derive(Debug, Error, Clone, JsError, PartialEq, Eq)]
pub enum PackageJsonDepValueParseErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  VersionReq(#[from] NpmVersionReqParseError),
  #[class(type)]
  #[error("Not implemented scheme '{scheme}'")]
  Unsupported { scheme: String },
  #[class(inherit)]
  #[error(transparent)]
  JsrRequiresScope(#[from] JsrDepPackageParseError),
  #[class(type)]
  #[error("Package name must not be empty")]
  EmptyName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PackageJsonDepWorkspaceReq {
  /// "workspace:~"
  Tilde,

  /// "workspace:^"
  Caret,

  /// "workspace:x.y.z", "workspace:*", "workspace:^x.y.z"
  VersionReq(VersionReq),
}

/// Error returned when a JSR specifier doesn't have a valid `@scope/name`
/// format.
#[derive(Debug, Clone, Error, JsError, PartialEq, Eq)]
#[class(type)]
#[error("JSR package name '{name}' requires a scope (e.g. @scope/name)")]
pub struct JsrDepPackageParseError {
  pub name: String,
}

/// Parses a JSR specifier value (the part after the `jsr:` prefix) into an
/// npm-style package name and version string.
///
/// If the value starts with `@`, it's parsed as `@scope/name[@version]`.
/// Otherwise, `fallback_name` is used as the JSR package name and the
/// value is treated as a version string.
pub fn parse_jsr_dep_value<'a>(
  fallback_name: &'a str,
  jsr_value: &'a str,
) -> Result<(StackString, &'a str), JsrDepPackageParseError> {
  let (jsr_name, version_str) = if jsr_value.starts_with('@') {
    if let Some((name, version)) = jsr_value.rsplit_once('@') {
      if name.is_empty() {
        (jsr_value, "*")
      } else {
        (name, version)
      }
    } else {
      (jsr_value, "*")
    }
  } else if let Some((name, version)) = jsr_value.split_once('@') {
    // unscoped name with version, e.g. "test@*"
    (name, version)
  } else {
    // bare version string, e.g. "^1" — derive name from key
    (fallback_name, jsr_value)
  };

  let Some((scope, name)) = jsr_name
    .strip_prefix('@')
    .and_then(|rest| rest.split_once('/'))
  else {
    return Err(JsrDepPackageParseError {
      name: jsr_name.to_string(),
    });
  };

  let npm_name =
    capacity_builder::StringBuilder::<StackString>::build(|builder| {
      builder.append("@jsr/");
      builder.append(scope);
      builder.append("__");
      builder.append(name);
    })
    .unwrap();

  Ok((npm_name, version_str))
}

/// A git dependency declared in `package.json`.
///
/// These come in many forms supported by npm/pnpm/yarn, e.g.:
///
/// - `git://github.com/user/repo.git`
/// - `git+ssh://git@github.com:user/repo.git#v1.0.0`
/// - `git+https://github.com/user/repo.git#semver:^1.0.0`
/// - `https://github.com/user/repo.git#semver:v2.29.0&path:/frontend`
/// - `github:user/repo`
/// - `user/repo` (GitHub shorthand)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitDep {
  /// The URL to clone the repository from, with any `git+` prefix removed and
  /// shorthand forms expanded (e.g. `github:user/repo` ->
  /// `https://github.com/user/repo.git`). The committish fragment is not
  /// included here.
  pub url: String,
  /// An explicit committish (branch, tag or commit hash) the dependency was
  /// pinned to via `#<committish>`. Mutually exclusive with `semver`.
  pub committish: Option<String>,
  /// A semver range the dependency was pinned to via `#semver:<range>`. The
  /// matching tag is resolved at install time.
  pub semver: Option<String>,
  /// A sub directory within the repository that contains the package, declared
  /// via the pnpm `&path:<path>` fragment. The leading slash (if any) is
  /// stripped.
  pub sub_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PackageJsonDepValue {
  File(String),
  /// A git dependency (see [`GitDep`]).
  Git(GitDep),
  Req(PackageReq),
  Workspace(PackageJsonDepWorkspaceReq),
  Catalog(String),
}

/// Returns `true` if a bare (scheme-less) dependency value looks like a GitHub
/// `owner/repo` shorthand (e.g. `denoland/deno`).
fn is_github_shorthand(value: &str) -> bool {
  let Some((owner, repo)) = value.split_once('/') else {
    return false;
  };
  if owner.is_empty() || repo.is_empty() || repo.contains('/') {
    return false;
  }
  fn is_valid_segment(segment: &str) -> bool {
    segment
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
  }
  is_valid_segment(owner) && is_valid_segment(repo)
}

/// Returns `true` if an `http(s)://` URL refers to a git repository (either it
/// ends in `.git` or is hosted on a well known git host).
fn is_git_http_url(url: &str) -> bool {
  if url.trim_end_matches('/').ends_with(".git") {
    return true;
  }
  let Some(rest) = url
    .strip_prefix("https://")
    .or_else(|| url.strip_prefix("http://"))
  else {
    return false;
  };
  let authority = rest.split(['/', '?']).next().unwrap_or("");
  // strip any userinfo (e.g. `user@host`)
  let host = authority.rsplit('@').next().unwrap_or(authority);
  // strip any port
  let host = host.split(':').next().unwrap_or(host);
  matches!(
    host,
    "github.com" | "www.github.com" | "gitlab.com" | "bitbucket.org"
  )
}

/// Attempts to parse a `package.json` dependency value as a git dependency,
/// returning `None` if it is not one.
fn parse_git_dep(value: &str) -> Option<GitDep> {
  let (url_part, fragment) = match value.split_once('#') {
    Some((url, fragment)) => (url, Some(fragment)),
    None => (value, None),
  };

  let url = if let Some((scheme, rest)) = url_part.split_once(':') {
    match scheme {
      // `git://host/path` is already a cloneable URL.
      "git" => url_part.to_string(),
      "git+ssh" => format!("ssh:{rest}"),
      "git+https" => format!("https:{rest}"),
      "git+http" => format!("http:{rest}"),
      "git+file" => format!("file:{rest}"),
      "github" => format!("https://github.com/{rest}.git"),
      "gitlab" => format!("https://gitlab.com/{rest}.git"),
      "bitbucket" => format!("https://bitbucket.org/{rest}.git"),
      "https" | "http" if is_git_http_url(url_part) => url_part.to_string(),
      _ => return None,
    }
  } else if is_github_shorthand(url_part) {
    format!("https://github.com/{url_part}.git")
  } else {
    return None;
  };

  let mut committish = None;
  let mut semver = None;
  let mut sub_path = None;
  if let Some(fragment) = fragment {
    for part in fragment.split('&') {
      if let Some(range) = part.strip_prefix("semver:") {
        semver = Some(range.to_string());
      } else if let Some(path) = part.strip_prefix("path:") {
        let path = path.trim_start_matches('/');
        if !path.is_empty() {
          sub_path = Some(path.to_string());
        }
      } else if !part.is_empty() && committish.is_none() && semver.is_none() {
        committish = Some(part.to_string());
      }
    }
  }

  Some(GitDep {
    url,
    committish,
    semver,
    sub_path,
  })
}

impl PackageJsonDepValue {
  pub fn parse(
    key: &str,
    value: &str,
  ) -> Result<Self, PackageJsonDepValueParseError> {
    fn from_name_and_version_req(
      name: StackString,
      version_req: &str,
    ) -> Result<PackageJsonDepValue, PackageJsonDepValueParseError> {
      if name.is_empty() {
        return Err(PackageJsonDepValueParseErrorKind::EmptyName.into_box());
      }
      match VersionReq::parse_from_npm(version_req) {
        Ok(version_req) => {
          Ok(PackageJsonDepValue::Req(PackageReq { name, version_req }))
        }
        Err(err) => {
          Err(PackageJsonDepValueParseErrorKind::VersionReq(err).into_box())
        }
      }
    }

    if key.is_empty() {
      return Err(PackageJsonDepValueParseErrorKind::EmptyName.into_box());
    }

    if let Some(git_dep) = parse_git_dep(value) {
      return Ok(Self::Git(git_dep));
    }

    if let Some((scheme, value)) = value.split_once(':') {
      match scheme {
        "file" => Ok(Self::File(value.to_string())),
        "jsr" => {
          let (npm_name, version_req) = parse_jsr_dep_value(key, value)
            .map_err(|e| {
              PackageJsonDepValueParseErrorKind::JsrRequiresScope(e).into_box()
            })?;
          from_name_and_version_req(npm_name, version_req)
        }
        "npm" => {
          if let Some((name, version)) = value.rsplit_once('@') {
            // if empty, then the name was scoped and there's no version
            if name.is_empty() {
              from_name_and_version_req(value.into(), "*")
            } else {
              from_name_and_version_req(name.into(), version)
            }
          } else {
            from_name_and_version_req(value.into(), "*")
          }
        }
        "catalog" => {
          let name = if value.is_empty() || value == "default" {
            "default".to_string()
          } else {
            value.to_string()
          };
          Ok(Self::Catalog(name))
        }
        "workspace" => {
          let workspace_req = match value {
            "~" => PackageJsonDepWorkspaceReq::Tilde,
            "^" => PackageJsonDepWorkspaceReq::Caret,
            _ => PackageJsonDepWorkspaceReq::VersionReq(
              VersionReq::parse_from_npm(value)?,
            ),
          };
          Ok(Self::Workspace(workspace_req))
        }
        scheme => Err(
          PackageJsonDepValueParseErrorKind::Unsupported {
            scheme: scheme.to_string(),
          }
          .into_box(),
        ),
      }
    } else {
      from_name_and_version_req(key.into(), value)
    }
  }
}

pub type PackageJsonDepsMap = IndexMap<
  StackString,
  Result<PackageJsonDepValue, PackageJsonDepValueParseError>,
>;

#[derive(Debug, Clone)]
pub struct PackageJsonDeps {
  pub dependencies: PackageJsonDepsMap,
  pub dev_dependencies: PackageJsonDepsMap,
}

impl PackageJsonDeps {
  /// Gets a package.json dependency entry by alias.
  pub fn get(
    &self,
    alias: &str,
  ) -> Option<&Result<PackageJsonDepValue, PackageJsonDepValueParseError>> {
    self
      .dependencies
      .get(alias)
      .or_else(|| self.dev_dependencies.get(alias))
  }
}

#[derive(Debug, Error, JsError)]
pub enum PackageJsonLoadError {
  #[class(inherit)]
  #[error("Failed reading '{}'.", .path.display())]
  Io {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Malformed package.json '{}'.", .path.display())]
  Deserialize {
    path: PathBuf,
    #[source]
    #[inherit]
    source: serde_json::Error,
  },
  #[error(
    "\"exports\" cannot contain some keys starting with '.' and some not.\nThe exports object must either be an object of package subpath keys\nor an object of main entry condition name keys only."
  )]
  #[class(type)]
  InvalidExports,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
  pub exports: Option<Map<String, Value>>,
  pub imports: Option<Map<String, Value>>,
  pub bin: Option<Value>,
  pub main: Option<String>,
  pub module: Option<String>,
  pub browser: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub browser_map: Option<IndexMap<String, BrowserMapEntry>>,
  pub name: Option<String>,
  pub version: Option<String>,
  #[serde(skip)]
  pub path: PathBuf,
  #[serde(rename = "type")]
  pub typ: String,
  pub types: Option<String>,
  pub types_versions: Option<Map<String, Value>>,
  pub dependencies: Option<IndexMap<String, String>>,
  pub bundle_dependencies: Option<Vec<String>>,
  pub dev_dependencies: Option<IndexMap<String, String>>,
  pub peer_dependencies: Option<IndexMap<String, String>>,
  pub peer_dependencies_meta: Option<Value>,
  pub optional_dependencies: Option<IndexMap<String, String>>,
  pub directories: Option<Map<String, Value>>,
  pub scripts: Option<IndexMap<String, String>>,
  pub workspaces: Option<Vec<String>>,
  pub os: Option<Vec<String>>,
  pub cpu: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub overrides: Option<Map<String, Value>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub catalog: Option<IndexMap<String, String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub catalogs: Option<IndexMap<String, IndexMap<String, String>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub side_effects: Option<PackageJsonSideEffects>,
  #[serde(skip_serializing)]
  resolved_deps: PackageJsonDepsRcCell,
}

impl PackageJson {
  pub fn load_from_path(
    sys: &impl FsRead,
    maybe_cache: Option<&dyn PackageJsonCache>,
    path: &Path,
  ) -> Result<Option<PackageJsonRc>, PackageJsonLoadError> {
    let cache_entry = maybe_cache
      .map(|c| c.get(path))
      .unwrap_or(PackageJsonCacheResult::NotCached);

    match cache_entry {
      PackageJsonCacheResult::Hit(item) => Ok(item),
      PackageJsonCacheResult::NotCached => {
        match sys.fs_read_to_string_lossy(path) {
          Ok(file_text) => {
            let pkg_json =
              PackageJson::load_from_string(path.to_path_buf(), &file_text)?;
            let pkg_json = deno_maybe_sync::new_rc(pkg_json);
            if let Some(cache) = maybe_cache {
              cache.set(path.to_path_buf(), Some(pkg_json.clone()));
            }
            Ok(Some(pkg_json))
          }
          Err(err) if err.kind() == ErrorKind::NotFound => {
            if let Some(cache) = maybe_cache {
              cache.set(path.to_path_buf(), None);
            }
            Ok(None)
          }
          Err(err) => Err(PackageJsonLoadError::Io {
            path: path.to_path_buf(),
            source: err,
          }),
        }
      }
    }
  }

  pub fn load_from_string(
    path: PathBuf,
    source: &str,
  ) -> Result<PackageJson, PackageJsonLoadError> {
    if source.trim().is_empty() {
      return Ok(PackageJson {
        path,
        main: None,
        name: None,
        version: None,
        module: None,
        browser: None,
        browser_map: None,
        typ: "none".to_string(),
        types: None,
        types_versions: None,
        exports: None,
        imports: None,
        bin: None,
        dependencies: None,
        bundle_dependencies: None,
        dev_dependencies: None,
        peer_dependencies: None,
        peer_dependencies_meta: None,
        optional_dependencies: None,
        directories: None,
        scripts: None,
        workspaces: None,
        os: None,
        cpu: None,
        overrides: None,
        catalog: None,
        catalogs: None,
        side_effects: None,
        resolved_deps: Default::default(),
      });
    }

    let package_json: Value = serde_json::from_str(source).map_err(|err| {
      PackageJsonLoadError::Deserialize {
        path: path.clone(),
        source: err,
      }
    })?;
    Self::load_from_value(path, package_json)
  }

  pub fn load_from_value(
    path: PathBuf,
    package_json: serde_json::Value,
  ) -> Result<PackageJson, PackageJsonLoadError> {
    fn parse_string_map(
      value: serde_json::Value,
    ) -> Option<IndexMap<String, String>> {
      if let Value::Object(map) = value {
        let mut result = IndexMap::with_capacity(map.len());
        for (k, v) in map {
          if let Some(v) = map_string(v) {
            result.insert(k, v);
          }
        }
        Some(result)
      } else {
        None
      }
    }

    fn map_object(value: serde_json::Value) -> Option<Map<String, Value>> {
      match value {
        Value::Object(v) => Some(v),
        _ => None,
      }
    }

    fn map_string(value: serde_json::Value) -> Option<String> {
      match value {
        Value::String(v) => Some(v),
        Value::Number(v) => Some(v.to_string()),
        _ => None,
      }
    }

    fn map_array(value: serde_json::Value) -> Option<Vec<Value>> {
      match value {
        Value::Array(v) => Some(v),
        _ => None,
      }
    }

    fn parse_string_array(value: serde_json::Value) -> Option<Vec<String>> {
      let value = map_array(value)?;
      let mut result = Vec::with_capacity(value.len());
      for v in value {
        if let Some(v) = map_string(v) {
          result.push(v);
        }
      }
      Some(result)
    }

    let mut package_json = match package_json {
      Value::Object(o) => o,
      _ => Default::default(),
    };
    let imports_val = package_json.remove("imports");
    let main_val = package_json.remove("main");
    let module_val = package_json.remove("module");
    let browser_val = package_json.remove("browser");
    let name_val = package_json.remove("name");
    let version_val = package_json.remove("version");
    let type_val = package_json.remove("type");
    let bin = package_json.remove("bin");
    let exports = package_json
      .remove("exports")
      .map(|exports| {
        if is_conditional_exports_main_sugar(&exports)? {
          let mut map = Map::new();
          map.insert(".".to_string(), exports);
          Ok::<_, PackageJsonLoadError>(Some(map))
        } else {
          Ok(map_object(exports))
        }
      })
      .transpose()?
      .flatten();

    let imports = imports_val.and_then(map_object);
    let main = main_val.and_then(map_string);
    let name = name_val.and_then(map_string);
    let version = version_val.and_then(map_string);
    let module = module_val.and_then(map_string);
    let (browser, browser_map) = match browser_val {
      Some(Value::String(s)) => (Some(s), None),
      Some(Value::Object(map)) => {
        let mut entries = IndexMap::with_capacity(map.len());
        for (k, v) in map {
          match v {
            Value::String(s) => {
              entries.insert(k, BrowserMapEntry::Replace(s));
            }
            Value::Bool(false) => {
              entries.insert(k, BrowserMapEntry::Disabled);
            }
            _ => {}
          }
        }
        if entries.is_empty() {
          (None, None)
        } else {
          (None, Some(entries))
        }
      }
      _ => (None, None),
    };

    let dependencies = package_json
      .remove("dependencies")
      .and_then(parse_string_map);
    let dev_dependencies = package_json
      .remove("devDependencies")
      .and_then(parse_string_map);
    let bundle_dependencies = package_json
      .remove("bundleDependencies")
      .or_else(|| package_json.remove("bundledDependencies"))
      .and_then(parse_string_array);
    let peer_dependencies = package_json
      .remove("peerDependencies")
      .and_then(parse_string_map);
    let peer_dependencies_meta = package_json.remove("peerDependenciesMeta");
    let optional_dependencies = package_json
      .remove("optionalDependencies")
      .and_then(parse_string_map);

    let directories: Option<Map<String, Value>> =
      package_json.remove("directories").and_then(map_object);
    let scripts: Option<IndexMap<String, String>> =
      package_json.remove("scripts").and_then(parse_string_map);

    // Ignore unknown types for forwards compatibility
    let typ = if let Some(t) = type_val {
      if let Some(t) = t.as_str() {
        if t != "module" && t != "commonjs" {
          "none".to_string()
        } else {
          t.to_string()
        }
      } else {
        "none".to_string()
      }
    } else {
      "none".to_string()
    };

    // for typescript, it looks for "typings" first, then "types"
    let types = package_json
      .remove("typings")
      .or_else(|| package_json.remove("types"))
      .and_then(map_string);
    let types_versions =
      package_json.remove("typesVersions").and_then(map_object);
    // workspaces can be either an array of globs or an object with
    // "packages" (array) and optionally "catalog"/"catalogs" sub-fields
    // (Bun/Yarn object form).
    let (workspaces, ws_catalog, ws_catalogs) =
      match package_json.remove("workspaces") {
        Some(Value::Array(arr)) => {
          (parse_string_array(Value::Array(arr)), None, None)
        }
        Some(Value::Object(mut obj)) => {
          let pkgs = obj.remove("packages").and_then(parse_string_array);
          let cat = obj.remove("catalog").and_then(parse_string_map);
          let cats = obj.remove("catalogs").and_then(|v| {
            if let Value::Object(map) = v {
              let mut result = IndexMap::with_capacity(map.len());
              for (k, v) in map {
                if let Some(inner) = parse_string_map(v) {
                  result.insert(k, inner);
                }
              }
              Some(result)
            } else {
              None
            }
          });
          (pkgs, cat, cats)
        }
        _ => (None, None, None),
      };
    let os = package_json.remove("os").and_then(parse_string_array);
    let cpu = package_json.remove("cpu").and_then(parse_string_array);
    let overrides = package_json.remove("overrides").and_then(map_object);
    let side_effects =
      package_json.remove("sideEffects").and_then(|v| match v {
        Value::Bool(b) => Some(PackageJsonSideEffects::Bool(b)),
        Value::Array(_) => {
          parse_string_array(v).map(PackageJsonSideEffects::Patterns)
        }
        _ => None,
      });
    // Top-level catalog/catalogs take precedence; fall back to those
    // extracted from the workspaces object form.
    let catalog = package_json
      .remove("catalog")
      .and_then(parse_string_map)
      .or(ws_catalog);
    let catalogs = package_json
      .remove("catalogs")
      .and_then(|v| {
        if let Value::Object(map) = v {
          let mut result = IndexMap::with_capacity(map.len());
          for (k, v) in map {
            if let Some(inner) = parse_string_map(v) {
              result.insert(k, inner);
            }
          }
          Some(result)
        } else {
          None
        }
      })
      .or(ws_catalogs);

    Ok(PackageJson {
      path,
      main,
      name,
      version,
      module,
      browser,
      browser_map,
      typ,
      types,
      types_versions,
      exports,
      imports,
      bin,
      dependencies,
      dev_dependencies,
      bundle_dependencies,
      peer_dependencies,
      peer_dependencies_meta,
      optional_dependencies,
      directories,
      scripts,
      workspaces,
      os,
      cpu,
      overrides,
      catalog,
      catalogs,
      side_effects,
      resolved_deps: Default::default(),
    })
  }

  pub fn specifier(&self) -> Url {
    deno_path_util::url_from_file_path(&self.path).unwrap()
  }

  pub fn dir_path(&self) -> &Path {
    self.path.parent().unwrap()
  }

  /// Resolve the package.json's dependencies.
  pub fn resolve_local_package_json_deps(&self) -> &PackageJsonDepsRc {
    fn get_map(deps: Option<&IndexMap<String, String>>) -> PackageJsonDepsMap {
      let Some(deps) = deps else {
        return Default::default();
      };
      let mut result = IndexMap::with_capacity(deps.len());
      for (key, value) in deps {
        result
          .entry(StackString::from(key.as_str()))
          .or_insert_with(|| PackageJsonDepValue::parse(key, value));
      }
      result
    }

    self.resolved_deps.get_or_init(|| {
      PackageJsonDepsRc::new(PackageJsonDeps {
        dependencies: get_map(self.dependencies.as_ref()),
        dev_dependencies: get_map(self.dev_dependencies.as_ref()),
      })
    })
  }

  pub fn resolve_default_bin_name(
    &self,
  ) -> Result<&str, MissingPkgJsonNameError> {
    let Some(name) = &self.name else {
      return Err(MissingPkgJsonNameError {
        pkg_json_path: self.path.clone(),
      });
    };
    let name = name.split("/").last().unwrap();
    Ok(name)
  }

  pub fn resolve_bins(
    &self,
  ) -> Result<PackageJsonBins, MissingPkgJsonNameError> {
    match &self.bin {
      Some(Value::String(path)) => {
        let name = self.resolve_default_bin_name()?;
        Ok(PackageJsonBins::Bins(BTreeMap::from([(
          name.to_string(),
          self.dir_path().join(path),
        )])))
      }
      Some(Value::Object(o)) => Ok(PackageJsonBins::Bins(
        o.iter()
          .filter_map(|(key, value)| {
            let Value::String(path) = value else {
              return None;
            };
            Some((key.clone(), self.dir_path().join(path)))
          })
          .collect::<BTreeMap<_, _>>(),
      )),
      _ => {
        let bin_directory =
          self.directories.as_ref().and_then(|d| d.get("bin"));
        match bin_directory {
          Some(Value::String(bin_dir)) => {
            let bin_dir = self.dir_path().join(bin_dir);
            Ok(PackageJsonBins::Directory(bin_dir))
          }
          _ => Ok(PackageJsonBins::Bins(Default::default())),
        }
      }
    }
  }
}

fn is_conditional_exports_main_sugar(
  exports: &Value,
) -> Result<bool, PackageJsonLoadError> {
  if exports.is_string() || exports.is_array() {
    return Ok(true);
  }

  if exports.is_null() || !exports.is_object() {
    return Ok(false);
  }

  let exports_obj = exports.as_object().unwrap();
  let mut is_conditional_sugar = false;
  let mut i = 0;
  for key in exports_obj.keys() {
    let cur_is_conditional_sugar = key.is_empty() || !key.starts_with('.');
    if i == 0 {
      is_conditional_sugar = cur_is_conditional_sugar;
      i += 1;
    } else if is_conditional_sugar != cur_is_conditional_sugar {
      return Err(PackageJsonLoadError::InvalidExports);
    }
  }

  Ok(is_conditional_sugar)
}

#[cfg(test)]
mod test {
  use std::error::Error;
  use std::path::PathBuf;

  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn null_exports_should_not_crash() {
    let package_json = PackageJson::load_from_string(
      PathBuf::from("/package.json"),
      r#"{ "exports": null }"#,
    )
    .unwrap();

    assert!(package_json.exports.is_none());
  }

  fn get_local_package_json_version_reqs_for_tests(
    package_json: &PackageJson,
  ) -> IndexMap<
    String,
    Result<PackageJsonDepValue, PackageJsonDepValueParseErrorKind>,
  > {
    let deps = package_json.resolve_local_package_json_deps();
    deps
      .dependencies
      .clone()
      .into_iter()
      .chain(deps.dev_dependencies.clone())
      .map(|(k, v)| {
        (
          k.to_string(),
          match v {
            Ok(v) => Ok(v),
            Err(err) => Err(err.into_kind()),
          },
        )
      })
      .collect::<IndexMap<_, _>>()
  }

  #[test]
  fn test_get_local_package_json_version_reqs() {
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([
      ("test".to_string(), "^1.2".to_string()),
      ("other".to_string(), "npm:package@~1.3".to_string()),
    ]));
    package_json.dev_dependencies = Some(IndexMap::from([
      ("package_b".to_string(), "~2.2".to_string()),
      ("other".to_string(), "^3.2".to_string()),
    ]));
    let deps = package_json.resolve_local_package_json_deps();
    assert_eq!(
      deps
        .dependencies
        .clone()
        .into_iter()
        .map(|d| (d.0, d.1.unwrap()))
        .collect::<Vec<_>>(),
      Vec::from([
        (
          "test".into(),
          PackageJsonDepValue::Req(PackageReq::from_str("test@^1.2").unwrap())
        ),
        (
          "other".into(),
          PackageJsonDepValue::Req(
            PackageReq::from_str("package@~1.3").unwrap()
          )
        ),
      ])
    );
    assert_eq!(
      deps
        .dev_dependencies
        .clone()
        .into_iter()
        .map(|d| (d.0, d.1.unwrap()))
        .collect::<Vec<_>>(),
      Vec::from([
        (
          "package_b".into(),
          PackageJsonDepValue::Req(
            PackageReq::from_str("package_b@~2.2").unwrap()
          )
        ),
        (
          "other".into(),
          PackageJsonDepValue::Req(PackageReq::from_str("other@^3.2").unwrap())
        ),
      ])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_empty_name() {
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([
      ("".to_string(), ".".to_string()),
      ("npm-empty".to_string(), "npm:".to_string()),
      ("ok".to_string(), "^1".to_string()),
    ]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      map.get("").unwrap().as_ref().unwrap_err(),
      &PackageJsonDepValueParseErrorKind::EmptyName,
    );
    assert_eq!(
      map.get("npm-empty").unwrap().as_ref().unwrap_err(),
      &PackageJsonDepValueParseErrorKind::EmptyName,
    );
    assert!(map.get("ok").unwrap().is_ok());
  }

  #[test]
  fn test_get_local_package_json_version_reqs_errors_non_npm_specifier() {
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([(
      "test".to_string(),
      "%*(#$%()".to_string(),
    )]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(map.len(), 1);
    let err = map.get("test").unwrap().as_ref().unwrap_err();
    assert_eq!(format!("{}", err), "Invalid version requirement");
    assert_eq!(
      format!("{}", err.source().unwrap()),
      concat!("Unexpected character.\n", "  %*(#$%()\n", "  ~")
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_range() {
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([(
      "test".to_string(),
      "1.x - 1.3".to_string(),
    )]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      map,
      IndexMap::from([(
        "test".to_string(),
        Ok(PackageJsonDepValue::Req(PackageReq {
          name: "test".into(),
          version_req: VersionReq::parse_from_npm("1.x - 1.3").unwrap()
        }))
      )])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_jsr() {
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([
      ("@denotest/foo".to_string(), "jsr:^1.2".to_string()),
      ("@std/path2".to_string(), "jsr:@std/path@1".to_string()),
      ("@std/fs".to_string(), "jsr:@std/fs".to_string()),
      ("no-scope".to_string(), "jsr:*".to_string()),
      ("no-scope2".to_string(), "jsr:test@*".to_string()),
      ("@denotest/tag".to_string(), "jsr:future-tag".to_string()),
    ]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      map,
      IndexMap::from([
        (
          "@denotest/foo".to_string(),
          Ok(PackageJsonDepValue::Req(PackageReq {
            name: "@jsr/denotest__foo".into(),
            version_req: VersionReq::parse_from_specifier("^1.2").unwrap()
          }))
        ),
        (
          "@std/path2".to_string(),
          Ok(PackageJsonDepValue::Req(PackageReq {
            name: "@jsr/std__path".into(),
            version_req: VersionReq::parse_from_specifier("1").unwrap()
          }))
        ),
        (
          "@std/fs".to_string(),
          Ok(PackageJsonDepValue::Req(PackageReq {
            name: "@jsr/std__fs".into(),
            version_req: VersionReq::parse_from_specifier("*").unwrap()
          }))
        ),
        (
          "no-scope".to_string(),
          Err(PackageJsonDepValueParseErrorKind::JsrRequiresScope(
            JsrDepPackageParseError {
              name: "no-scope".to_string()
            }
          ))
        ),
        (
          "no-scope2".to_string(),
          Err(PackageJsonDepValueParseErrorKind::JsrRequiresScope(
            JsrDepPackageParseError {
              name: "test".to_string()
            }
          ))
        ),
        (
          "@denotest/tag".to_string(),
          Ok(PackageJsonDepValue::Req(PackageReq {
            name: "@jsr/denotest__tag".into(),
            version_req: VersionReq::parse_from_specifier("future-tag")
              .unwrap()
          }))
        ),
      ])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_skips_certain_specifiers() {
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([
      ("test".to_string(), "1".to_string()),
      (
        "work-test-version-req".to_string(),
        "workspace:1.1.1".to_string(),
      ),
      ("work-test-star".to_string(), "workspace:*".to_string()),
      ("work-test-tilde".to_string(), "workspace:~".to_string()),
      ("work-test-caret".to_string(), "workspace:^".to_string()),
      ("catalog-test".to_string(), "catalog:".to_string()),
      (
        "catalog-default-test".to_string(),
        "catalog:default".to_string(),
      ),
      (
        "catalog-named-test".to_string(),
        "catalog:react18".to_string(),
      ),
      ("file-test".to_string(), "file:something".to_string()),
      ("git-test".to_string(), "git:something".to_string()),
      ("http-test".to_string(), "http://something".to_string()),
      ("https-test".to_string(), "https://something".to_string()),
    ]));
    let result = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      result,
      IndexMap::from([
        (
          "test".to_string(),
          Ok(PackageJsonDepValue::Req(
            PackageReq::from_str("test@1").unwrap()
          ))
        ),
        (
          "work-test-star".to_string(),
          Ok(PackageJsonDepValue::Workspace(
            PackageJsonDepWorkspaceReq::VersionReq(
              VersionReq::parse_from_npm("*").unwrap()
            )
          ))
        ),
        (
          "work-test-version-req".to_string(),
          Ok(PackageJsonDepValue::Workspace(
            PackageJsonDepWorkspaceReq::VersionReq(
              VersionReq::parse_from_npm("1.1.1").unwrap()
            )
          ))
        ),
        (
          "work-test-tilde".to_string(),
          Ok(PackageJsonDepValue::Workspace(
            PackageJsonDepWorkspaceReq::Tilde
          ))
        ),
        (
          "work-test-caret".to_string(),
          Ok(PackageJsonDepValue::Workspace(
            PackageJsonDepWorkspaceReq::Caret
          ))
        ),
        (
          "catalog-test".to_string(),
          Ok(PackageJsonDepValue::Catalog("default".to_string())),
        ),
        (
          "catalog-default-test".to_string(),
          Ok(PackageJsonDepValue::Catalog("default".to_string())),
        ),
        (
          "catalog-named-test".to_string(),
          Ok(PackageJsonDepValue::Catalog("react18".to_string())),
        ),
        (
          "file-test".to_string(),
          Ok(PackageJsonDepValue::File("something".to_string())),
        ),
        (
          "git-test".to_string(),
          Ok(PackageJsonDepValue::Git(GitDep {
            url: "git:something".to_string(),
            committish: None,
            semver: None,
            sub_path: None,
          })),
        ),
        (
          "http-test".to_string(),
          Err(PackageJsonDepValueParseErrorKind::Unsupported {
            scheme: "http".to_string()
          }),
        ),
        (
          "https-test".to_string(),
          Err(PackageJsonDepValueParseErrorKind::Unsupported {
            scheme: "https".to_string()
          }),
        ),
      ])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_git() {
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([
      (
        "issue-example".to_string(),
        "https://github.com/opendatahub-io/odh-dashboard.git#semver:v2.29.0&path:/frontend".to_string(),
      ),
      (
        "git-proto".to_string(),
        "git://github.com/user/repo.git".to_string(),
      ),
      (
        "git-ssh".to_string(),
        "git+ssh://git@github.com:user/repo.git#main".to_string(),
      ),
      (
        "git-https-semver".to_string(),
        "git+https://github.com/user/repo.git#semver:^1.0.0".to_string(),
      ),
      (
        "github-scheme".to_string(),
        "github:user/repo#v1.2.3".to_string(),
      ),
      (
        "gitlab-scheme".to_string(),
        "gitlab:user/repo".to_string(),
      ),
      (
        "bitbucket-scheme".to_string(),
        "bitbucket:user/repo".to_string(),
      ),
      (
        "github-shorthand".to_string(),
        "user/repo".to_string(),
      ),
      (
        "github-host-no-dot-git".to_string(),
        "https://github.com/user/repo".to_string(),
      ),
    ]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      map.get("issue-example").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "https://github.com/opendatahub-io/odh-dashboard.git".to_string(),
        committish: None,
        semver: Some("v2.29.0".to_string()),
        sub_path: Some("frontend".to_string()),
      })
    );
    assert_eq!(
      map.get("git-proto").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "git://github.com/user/repo.git".to_string(),
        committish: None,
        semver: None,
        sub_path: None,
      })
    );
    assert_eq!(
      map.get("git-ssh").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "ssh://git@github.com:user/repo.git".to_string(),
        committish: Some("main".to_string()),
        semver: None,
        sub_path: None,
      })
    );
    assert_eq!(
      map.get("git-https-semver").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "https://github.com/user/repo.git".to_string(),
        committish: None,
        semver: Some("^1.0.0".to_string()),
        sub_path: None,
      })
    );
    assert_eq!(
      map.get("github-scheme").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "https://github.com/user/repo.git".to_string(),
        committish: Some("v1.2.3".to_string()),
        semver: None,
        sub_path: None,
      })
    );
    assert_eq!(
      map.get("gitlab-scheme").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "https://gitlab.com/user/repo.git".to_string(),
        committish: None,
        semver: None,
        sub_path: None,
      })
    );
    assert_eq!(
      map.get("bitbucket-scheme").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "https://bitbucket.org/user/repo.git".to_string(),
        committish: None,
        semver: None,
        sub_path: None,
      })
    );
    assert_eq!(
      map.get("github-shorthand").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "https://github.com/user/repo.git".to_string(),
        committish: None,
        semver: None,
        sub_path: None,
      })
    );
    assert_eq!(
      map.get("github-host-no-dot-git").unwrap().as_ref().unwrap(),
      &PackageJsonDepValue::Git(GitDep {
        url: "https://github.com/user/repo".to_string(),
        committish: None,
        semver: None,
        sub_path: None,
      })
    );
  }

  #[test]
  fn test_git_dep_does_not_match_non_git() {
    // bare semver ranges, npm aliases and remote tarballs must not be parsed
    // as git dependencies.
    let mut package_json =
      PackageJson::load_from_string(PathBuf::from("/package.json"), "{}")
        .unwrap();
    package_json.dependencies = Some(IndexMap::from([
      ("range".to_string(), "1.x - 1.3".to_string()),
      ("scoped".to_string(), "npm:@scope/name@1".to_string()),
      (
        "tarball".to_string(),
        "https://example.com/foo.tgz".to_string(),
      ),
    ]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert!(matches!(
      map.get("range").unwrap().as_ref().unwrap(),
      PackageJsonDepValue::Req(_)
    ));
    assert!(matches!(
      map.get("scoped").unwrap().as_ref().unwrap(),
      PackageJsonDepValue::Req(_)
    ));
    // a remote (non-git) tarball is still unsupported, not a git dep
    assert_eq!(
      map.get("tarball").unwrap().as_ref().unwrap_err(),
      &PackageJsonDepValueParseErrorKind::Unsupported {
        scheme: "https".to_string()
      },
    );
  }

  #[test]
  fn test_deserialize_serialize() {
    let json_value = serde_json::json!({
      "name": "test",
      "version": "1",
      "exports": {
        ".": "./main.js",
      },
      "bin": "./main.js",
      "types": "./types.d.ts",
      "typesVersions": {
        "<4.0": { "index.d.ts": ["index.v3.d.ts"] }
      },
      "imports": {
        "#test": "./main.js",
      },
      "main": "./main.js",
      "module": "./module.js",
      "browser": "./browser.js",
      "type": "module",
      "dependencies": {
        "name": "1.2",
      },
      "directories": {
        "bin": "./bin",
      },
      "devDependencies": {
        "name": "1.2",
      },
      "scripts": {
        "test": "echo \"Error: no test specified\" && exit 1",
      },
      "workspaces": ["asdf", "asdf2"],
      "cpu": ["x86_64"],
      "os": ["win32"],
      "optionalDependencies": {
        "optional": "1.1"
      },
      "bundleDependencies": [
        "name"
      ],
      "peerDependencies": {
        "peer": "1.0"
      },
      "peerDependenciesMeta": {
        "peer": {
          "optional": true
        }
      },
    });
    let package_json = PackageJson::load_from_value(
      PathBuf::from("/package.json"),
      json_value.clone(),
    )
    .unwrap();
    let serialized_value = serde_json::to_value(&package_json).unwrap();
    assert_eq!(serialized_value, json_value);
  }

  // https://github.com/denoland/deno/issues/26031
  #[test]
  fn test_exports_error() {
    let json_value = serde_json::json!({
      "name": "test",
      "version": "1",
      "exports": { ".": "./a", "a": "./a" },
    });
    assert!(matches!(
      PackageJson::load_from_value(
        PathBuf::from("/package.json"),
        json_value.clone(),
      ),
      Err(PackageJsonLoadError::InvalidExports)
    ));
  }

  #[test]
  fn test_workspaces_object_form_catalog() {
    let json_value = serde_json::json!({
      "workspaces": {
        "packages": ["packages/*"],
        "catalog": {
          "@types/bun": "1.3.12",
          "@types/node": "22.13.9"
        }
      }
    });
    let pj =
      PackageJson::load_from_value(PathBuf::from("/package.json"), json_value)
        .unwrap();
    assert_eq!(pj.workspaces, Some(vec!["packages/*".to_string()]));
    let catalog = pj.catalog.unwrap();
    assert_eq!(catalog.get("@types/bun").unwrap(), "1.3.12");
    assert_eq!(catalog.get("@types/node").unwrap(), "22.13.9");
  }

  #[test]
  fn test_workspaces_object_form_top_level_catalog_takes_precedence() {
    let json_value = serde_json::json!({
      "catalog": {
        "foo": "1.0.0"
      },
      "workspaces": {
        "packages": ["packages/*"],
        "catalog": {
          "foo": "2.0.0"
        }
      }
    });
    let pj =
      PackageJson::load_from_value(PathBuf::from("/package.json"), json_value)
        .unwrap();
    // Top-level catalog should win
    let catalog = pj.catalog.unwrap();
    assert_eq!(catalog.get("foo").unwrap(), "1.0.0");
  }
}
