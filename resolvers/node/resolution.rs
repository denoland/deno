// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Error as AnyError;
use deno_media_type::MediaType;
use deno_package_json::PackageJsonRc;
use deno_path_util::strip_unc_prefix;
use deno_path_util::url_from_file_path;
use serde_json::Map;
use serde_json::Value;
use url::Url;

use crate::env::NodeResolverEnv;
use crate::errors;
use crate::errors::CanonicalizingPkgJsonDirError;
use crate::errors::ClosestPkgJsonError;
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
use crate::errors::PackageJsonLoadError;
use crate::errors::PackagePathNotExportedError;
use crate::errors::PackageResolveError;
use crate::errors::PackageSubpathResolveError;
use crate::errors::PackageSubpathResolveErrorKind;
use crate::errors::PackageTargetNotFoundError;
use crate::errors::PackageTargetResolveError;
use crate::errors::PackageTargetResolveErrorKind;
use crate::errors::ResolveBinaryCommandsError;
use crate::errors::ResolvePkgJsonBinExportError;
use crate::errors::ResolvePkgSubpathFromDenoModuleError;
use crate::errors::TypeScriptNotSupportedInNpmError;
use crate::errors::TypesNotFoundError;
use crate::errors::TypesNotFoundErrorData;
use crate::errors::UnsupportedDirImportError;
use crate::errors::UnsupportedEsmUrlSchemeError;
use crate::errors::UrlToNodeResolutionError;
use crate::NpmResolverRc;
use crate::PathClean;
use deno_package_json::PackageJson;

pub static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];
pub static REQUIRE_CONDITIONS: &[&str] = &["require", "node"];
static TYPES_ONLY_CONDITIONS: &[&str] = &["types"];

pub type NodeModuleKind = deno_package_json::NodeModuleKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeResolutionMode {
  Execution,
  Types,
}

impl NodeResolutionMode {
  pub fn is_types(&self) -> bool {
    matches!(self, NodeResolutionMode::Types)
  }
}

#[derive(Debug)]
pub enum NodeResolution {
  Esm(Url),
  CommonJs(Url),
  BuiltIn(String),
}

impl NodeResolution {
  pub fn into_url(self) -> Url {
    match self {
      Self::Esm(u) => u,
      Self::CommonJs(u) => u,
      Self::BuiltIn(specifier) => {
        if specifier.starts_with("node:") {
          Url::parse(&specifier).unwrap()
        } else {
          Url::parse(&format!("node:{specifier}")).unwrap()
        }
      }
    }
  }

  pub fn into_specifier_and_media_type(
    resolution: Option<Self>,
  ) -> (Url, MediaType) {
    match resolution {
      Some(NodeResolution::CommonJs(specifier)) => {
        let media_type = MediaType::from_specifier(&specifier);
        (
          specifier,
          match media_type {
            MediaType::JavaScript | MediaType::Jsx => MediaType::Cjs,
            MediaType::TypeScript | MediaType::Tsx => MediaType::Cts,
            MediaType::Dts => MediaType::Dcts,
            _ => media_type,
          },
        )
      }
      Some(NodeResolution::Esm(specifier)) => {
        let media_type = MediaType::from_specifier(&specifier);
        (
          specifier,
          match media_type {
            MediaType::JavaScript | MediaType::Jsx => MediaType::Mjs,
            MediaType::TypeScript | MediaType::Tsx => MediaType::Mts,
            MediaType::Dts => MediaType::Dmts,
            _ => media_type,
          },
        )
      }
      Some(resolution) => (resolution.into_url(), MediaType::Dts),
      None => (
        Url::parse("internal:///missing_dependency.d.ts").unwrap(),
        MediaType::Dts,
      ),
    }
  }
}

#[allow(clippy::disallowed_types)]
pub type NodeResolverRc<TEnv> = crate::sync::MaybeArc<NodeResolver<TEnv>>;

#[derive(Debug)]
pub struct NodeResolver<TEnv: NodeResolverEnv> {
  env: TEnv,
  npm_resolver: NpmResolverRc,
}

impl<TEnv: NodeResolverEnv> NodeResolver<TEnv> {
  pub fn new(env: TEnv, npm_resolver: NpmResolverRc) -> Self {
    Self { env, npm_resolver }
  }

  pub fn in_npm_package(&self, specifier: &Url) -> bool {
    self.npm_resolver.in_npm_package(specifier)
  }

  /// This function is an implementation of `defaultResolve` in
  /// `lib/internal/modules/esm/resolve.js` from Node.
  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &Url,
    referrer_kind: NodeModuleKind,
    mode: NodeResolutionMode,
  ) -> Result<NodeResolution, NodeResolveError> {
    // Note: if we are here, then the referrer is an esm module
    // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

    if self.env.is_builtin_node_module(specifier) {
      return Ok(NodeResolution::BuiltIn(specifier.to_string()));
    }

    if let Ok(url) = Url::parse(specifier) {
      if url.scheme() == "data" {
        return Ok(NodeResolution::Esm(url));
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
        return Ok(NodeResolution::Esm(url));
      }
    }

    let url = self.module_resolve(
      specifier,
      referrer,
      referrer_kind,
      // even though the referrer may be CJS, if we're here that means we're doing ESM resolution
      DEFAULT_CONDITIONS,
      mode,
    )?;

    let url = if mode.is_types() {
      let file_path = to_file_path(&url);
      self.path_to_declaration_url(&file_path, Some(referrer), referrer_kind)?
    } else {
      url
    };

    let url = self.finalize_resolution(url, Some(referrer))?;
    let resolve_response = self.url_to_node_resolution(url)?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(resolve_response)
  }

  fn module_resolve(
    &self,
    specifier: &str,
    referrer: &Url,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, NodeResolveError> {
    if should_be_treated_as_relative_or_absolute_path(specifier) {
      Ok(referrer.join(specifier).map_err(|err| {
        NodeResolveRelativeJoinError {
          path: specifier.to_string(),
          base: referrer.clone(),
          source: err,
        }
      })?)
    } else if specifier.starts_with('#') {
      let pkg_config = self
        .get_closest_package_json(referrer)
        .map_err(PackageImportsResolveErrorKind::ClosestPkgJson)
        .map_err(|err| PackageImportsResolveError(Box::new(err)))?;
      Ok(self.package_imports_resolve(
        specifier,
        Some(referrer),
        referrer_kind,
        pkg_config.as_deref(),
        conditions,
        mode,
      )?)
    } else if let Ok(resolved) = Url::parse(specifier) {
      Ok(resolved)
    } else {
      Ok(self.package_resolve(
        specifier,
        referrer,
        referrer_kind,
        conditions,
        mode,
      )?)
    }
  }

  fn finalize_resolution(
    &self,
    resolved: Url,
    maybe_referrer: Option<&Url>,
  ) -> Result<Url, FinalizeResolutionError> {
    let encoded_sep_re = lazy_regex::regex!(r"%2F|%2C");

    if encoded_sep_re.is_match(resolved.path()) {
      return Err(
        errors::InvalidModuleSpecifierError {
          request: resolved.to_string(),
          reason: Cow::Borrowed(
            "must not include encoded \"/\" or \"\\\\\" characters",
          ),
          maybe_referrer: maybe_referrer.map(to_file_path_string),
        }
        .into(),
      );
    }

    if resolved.scheme() == "node" {
      return Ok(resolved);
    }

    let path = to_file_path(&resolved);

    // TODO(bartlomieju): currently not supported
    // if (getOptionValue('--experimental-specifier-resolution') === 'node') {
    //   ...
    // }

    let p_str = path.to_str().unwrap();
    let p = if p_str.ends_with('/') {
      p_str[p_str.len() - 1..].to_string()
    } else {
      p_str.to_string()
    };

    let (is_dir, is_file) = if let Ok(stats) = self.env.stat_sync(Path::new(&p))
    {
      (stats.is_dir, stats.is_file)
    } else {
      (false, false)
    };
    if is_dir {
      return Err(
        UnsupportedDirImportError {
          dir_url: resolved.clone(),
          maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
        }
        .into(),
      );
    } else if !is_file {
      return Err(
        ModuleNotFoundError {
          specifier: resolved,
          maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
          typ: "module",
        }
        .into(),
      );
    }

    Ok(resolved)
  }

  pub fn resolve_package_subpath_from_deno_module(
    &self,
    package_dir: &Path,
    package_subpath: Option<&str>,
    maybe_referrer: Option<&Url>,
    mode: NodeResolutionMode,
  ) -> Result<NodeResolution, ResolvePkgSubpathFromDenoModuleError> {
    let node_module_kind = NodeModuleKind::Esm;
    let package_subpath = package_subpath
      .map(|s| format!("./{s}"))
      .unwrap_or_else(|| ".".to_string());
    let resolved_url = self.resolve_package_dir_subpath(
      package_dir,
      &package_subpath,
      maybe_referrer,
      node_module_kind,
      DEFAULT_CONDITIONS,
      mode,
    )?;
    let resolve_response = self.url_to_node_resolution(resolved_url)?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(resolve_response)
  }

  pub fn resolve_binary_commands(
    &self,
    package_folder: &Path,
  ) -> Result<Vec<String>, ResolveBinaryCommandsError> {
    let pkg_json_path = package_folder.join("package.json");
    let Some(package_json) = self.load_package_json(&pkg_json_path)? else {
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
  ) -> Result<NodeResolution, ResolvePkgJsonBinExportError> {
    let pkg_json_path = package_folder.join("package.json");
    let Some(package_json) = self.load_package_json(&pkg_json_path)? else {
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
    let url = url_from_file_path(&package_folder.join(bin_entry)).unwrap();

    let resolve_response = self.url_to_node_resolution(url)?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(resolve_response)
  }

  pub fn url_to_node_resolution(
    &self,
    url: Url,
  ) -> Result<NodeResolution, UrlToNodeResolutionError> {
    let url_str = url.as_str().to_lowercase();
    if url_str.starts_with("http") || url_str.ends_with(".json") {
      Ok(NodeResolution::Esm(url))
    } else if url_str.ends_with(".js") || url_str.ends_with(".d.ts") {
      let maybe_package_config = self.get_closest_package_json(&url)?;
      match maybe_package_config {
        Some(c) if c.typ == "module" => Ok(NodeResolution::Esm(url)),
        Some(_) => Ok(NodeResolution::CommonJs(url)),
        None => Ok(NodeResolution::Esm(url)),
      }
    } else if url_str.ends_with(".mjs") || url_str.ends_with(".d.mts") {
      Ok(NodeResolution::Esm(url))
    } else if url_str.ends_with(".ts") || url_str.ends_with(".mts") {
      if self.in_npm_package(&url) {
        Err(TypeScriptNotSupportedInNpmError { specifier: url }.into())
      } else {
        Ok(NodeResolution::Esm(url))
      }
    } else {
      Ok(NodeResolution::CommonJs(url))
    }
  }

  /// Checks if the resolved file has a corresponding declaration file.
  fn path_to_declaration_url(
    &self,
    path: &Path,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
  ) -> Result<Url, TypesNotFoundError> {
    fn probe_extensions<TEnv: NodeResolverEnv>(
      fs: &TEnv,
      path: &Path,
      lowercase_path: &str,
      referrer_kind: NodeModuleKind,
    ) -> Option<PathBuf> {
      let mut searched_for_d_mts = false;
      let mut searched_for_d_cts = false;
      if lowercase_path.ends_with(".mjs") {
        let d_mts_path = with_known_extension(path, "d.mts");
        if fs.exists_sync(&d_mts_path) {
          return Some(d_mts_path);
        }
        searched_for_d_mts = true;
      } else if lowercase_path.ends_with(".cjs") {
        let d_cts_path = with_known_extension(path, "d.cts");
        if fs.exists_sync(&d_cts_path) {
          return Some(d_cts_path);
        }
        searched_for_d_cts = true;
      }

      let dts_path = with_known_extension(path, "d.ts");
      if fs.exists_sync(&dts_path) {
        return Some(dts_path);
      }

      let specific_dts_path = match referrer_kind {
        NodeModuleKind::Cjs if !searched_for_d_cts => {
          Some(with_known_extension(path, "d.cts"))
        }
        NodeModuleKind::Esm if !searched_for_d_mts => {
          Some(with_known_extension(path, "d.mts"))
        }
        _ => None, // already searched above
      };
      if let Some(specific_dts_path) = specific_dts_path {
        if fs.exists_sync(&specific_dts_path) {
          return Some(specific_dts_path);
        }
      }
      None
    }

    let lowercase_path = path.to_string_lossy().to_lowercase();
    if lowercase_path.ends_with(".d.ts")
      || lowercase_path.ends_with(".d.cts")
      || lowercase_path.ends_with(".d.mts")
    {
      return Ok(url_from_file_path(path).unwrap());
    }
    if let Some(path) =
      probe_extensions(&self.env, path, &lowercase_path, referrer_kind)
    {
      return Ok(url_from_file_path(&path).unwrap());
    }
    if self.env.is_dir_sync(path) {
      let resolution_result = self.resolve_package_dir_subpath(
        path,
        /* sub path */ ".",
        maybe_referrer,
        referrer_kind,
        match referrer_kind {
          NodeModuleKind::Esm => DEFAULT_CONDITIONS,
          NodeModuleKind::Cjs => REQUIRE_CONDITIONS,
        },
        NodeResolutionMode::Types,
      );
      if let Ok(resolution) = resolution_result {
        return Ok(resolution);
      }
      let index_path = path.join("index.js");
      if let Some(path) = probe_extensions(
        &self.env,
        &index_path,
        &index_path.to_string_lossy().to_lowercase(),
        referrer_kind,
      ) {
        return Ok(url_from_file_path(&path).unwrap());
      }
    }
    // allow resolving .css files for types resolution
    if lowercase_path.ends_with(".css") {
      return Ok(url_from_file_path(path).unwrap());
    }
    Err(TypesNotFoundError(Box::new(TypesNotFoundErrorData {
      code_specifier: url_from_file_path(path).unwrap(),
      maybe_referrer: maybe_referrer.cloned(),
    })))
  }

  #[allow(clippy::too_many_arguments)]
  pub fn package_imports_resolve(
    &self,
    name: &str,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    referrer_pkg_json: Option<&PackageJson>,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageImportsResolveError> {
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
            referrer_kind,
            false,
            true,
            conditions,
            mode,
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
              referrer_kind,
              true,
              true,
              conditions,
              mode,
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
        maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
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
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageTargetResolveError> {
    if !subpath.is_empty() && !pattern && !target.ends_with('/') {
      return Err(
        InvalidPackageTargetError {
          pkg_json_path: package_json_path.to_path_buf(),
          sub_path: match_.to_string(),
          target: target.to_string(),
          is_import: internal,
          maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
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
              return Ok(url);
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
            let package_json_url =
              url_from_file_path(package_json_path).unwrap();
            let result = match self.package_resolve(
              &export_target,
              &package_json_url,
              referrer_kind,
              conditions,
              mode,
            ) {
              Ok(url) => Ok(url),
              Err(err) => match err.code() {
                NodeJsErrorCode::ERR_INVALID_MODULE_SPECIFIER
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
                      maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
                      referrer_kind,
                      mode,
                    },
                  )
                  .into(),
                ),
              },
            };

            return match result {
              Ok(url) => Ok(url),
              Err(err) => {
                if self.env.is_builtin_node_module(target) {
                  Ok(Url::parse(&format!("node:{}", target)).unwrap())
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
          maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
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
          maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
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
          maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
        }
        .into(),
      );
    }
    if subpath.is_empty() {
      return Ok(url_from_file_path(&resolved_path).unwrap());
    }
    if invalid_segment_re.is_match(subpath) {
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
    }
    if pattern {
      let resolved_path_str = resolved_path.to_string_lossy();
      let replaced = pattern_re
        .replace(&resolved_path_str, |_caps: &regex::Captures| subpath);
      return Ok(
        url_from_file_path(&PathBuf::from(replaced.to_string())).unwrap(),
      );
    }
    Ok(url_from_file_path(&resolved_path.join(subpath).clean()).unwrap())
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_target(
    &self,
    package_json_path: &Path,
    target: &Value,
    subpath: &str,
    package_subpath: &str,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Option<Url>, PackageTargetResolveError> {
    let result = self.resolve_package_target_inner(
      package_json_path,
      target,
      subpath,
      package_subpath,
      maybe_referrer,
      referrer_kind,
      pattern,
      internal,
      conditions,
      mode,
    );
    match result {
      Ok(maybe_resolved) => Ok(maybe_resolved),
      Err(err) => {
        if mode.is_types()
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
            referrer_kind,
            pattern,
            internal,
            TYPES_ONLY_CONDITIONS,
            mode,
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
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Option<Url>, PackageTargetResolveError> {
    if let Some(target) = target.as_str() {
      let url = self.resolve_package_target_string(
        target,
        subpath,
        package_subpath,
        package_json_path,
        maybe_referrer,
        referrer_kind,
        pattern,
        internal,
        conditions,
        mode,
      )?;
      if mode.is_types() && url.scheme() == "file" {
        let path = deno_path_util::url_to_file_path(&url).unwrap();
        return Ok(Some(self.path_to_declaration_url(
          &path,
          maybe_referrer,
          referrer_kind,
        )?));
      } else {
        return Ok(Some(url));
      }
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
          referrer_kind,
          pattern,
          internal,
          conditions,
          mode,
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
      for key in target_obj.keys() {
        // TODO(bartlomieju): verify that keys are not numeric
        // return Err(errors::err_invalid_package_config(
        //   to_file_path_string(package_json_url),
        //   Some(base.as_str().to_string()),
        //   Some("\"exports\" cannot contain numeric property keys.".to_string()),
        // ));

        if key == "default"
          || conditions.contains(&key.as_str())
          || mode.is_types() && key.as_str() == "types"
        {
          let condition_target = target_obj.get(key).unwrap();

          let resolved = self.resolve_package_target(
            package_json_path,
            condition_target,
            subpath,
            package_subpath,
            maybe_referrer,
            referrer_kind,
            pattern,
            internal,
            conditions,
            mode,
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
        maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
      }
      .into(),
    )
  }

  #[allow(clippy::too_many_arguments)]
  pub fn package_exports_resolve(
    &self,
    package_json_path: &Path,
    package_subpath: &str,
    package_exports: &Map<String, Value>,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageExportsResolveError> {
    if package_exports.contains_key(package_subpath)
      && package_subpath.find('*').is_none()
      && !package_subpath.ends_with('/')
    {
      let target = package_exports.get(package_subpath).unwrap();
      let resolved = self.resolve_package_target(
        package_json_path,
        target,
        "",
        package_subpath,
        maybe_referrer,
        referrer_kind,
        false,
        false,
        conditions,
        mode,
      )?;
      return match resolved {
        Some(resolved) => Ok(resolved),
        None => Err(
          PackagePathNotExportedError {
            pkg_json_path: package_json_path.to_path_buf(),
            subpath: package_subpath.to_string(),
            maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
            mode,
          }
          .into(),
        ),
      };
    }

    let mut best_match = "";
    let mut best_match_subpath = None;
    for key in package_exports.keys() {
      let pattern_index = key.find('*');
      if let Some(pattern_index) = pattern_index {
        let key_sub = &key[0..pattern_index];
        if package_subpath.starts_with(key_sub) {
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
            best_match_subpath = Some(
              package_subpath[pattern_index
                ..(package_subpath.len() - pattern_trailer.len())]
                .to_string(),
            );
          }
        }
      }
    }

    if !best_match.is_empty() {
      let target = package_exports.get(best_match).unwrap();
      let maybe_resolved = self.resolve_package_target(
        package_json_path,
        target,
        &best_match_subpath.unwrap(),
        best_match,
        maybe_referrer,
        referrer_kind,
        true,
        false,
        conditions,
        mode,
      )?;
      if let Some(resolved) = maybe_resolved {
        return Ok(resolved);
      } else {
        return Err(
          PackagePathNotExportedError {
            pkg_json_path: package_json_path.to_path_buf(),
            subpath: package_subpath.to_string(),
            maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
            mode,
          }
          .into(),
        );
      }
    }

    Err(
      PackagePathNotExportedError {
        pkg_json_path: package_json_path.to_path_buf(),
        subpath: package_subpath.to_string(),
        maybe_referrer: maybe_referrer.map(ToOwned::to_owned),
        mode,
      }
      .into(),
    )
  }

  pub(super) fn package_resolve(
    &self,
    specifier: &str,
    referrer: &Url,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageResolveError> {
    let (package_name, package_subpath, _is_scoped) =
      parse_npm_pkg_name(specifier, referrer)?;

    if let Some(package_config) = self.get_closest_package_json(referrer)? {
      // ResolveSelf
      if package_config.name.as_ref() == Some(&package_name) {
        if let Some(exports) = &package_config.exports {
          return self
            .package_exports_resolve(
              &package_config.path,
              &package_subpath,
              exports,
              Some(referrer),
              referrer_kind,
              conditions,
              mode,
            )
            .map_err(|err| err.into());
        }
      }
    }

    self.resolve_package_subpath_for_package(
      &package_name,
      &package_subpath,
      referrer,
      referrer_kind,
      conditions,
      mode,
    )
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_subpath_for_package(
    &self,
    package_name: &str,
    package_subpath: &str,
    referrer: &Url,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageResolveError> {
    let result = self.resolve_package_subpath_for_package_inner(
      package_name,
      package_subpath,
      referrer,
      referrer_kind,
      conditions,
      mode,
    );
    if mode.is_types() && !matches!(result, Ok(Url { .. })) {
      // try to resolve with the @types package
      let package_name = types_package_name(package_name);
      if let Ok(result) = self.resolve_package_subpath_for_package_inner(
        &package_name,
        package_subpath,
        referrer,
        referrer_kind,
        conditions,
        mode,
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
    referrer: &Url,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageResolveError> {
    let package_dir_path = self
      .npm_resolver
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
        referrer_kind,
        conditions,
        mode,
      )
      .map_err(|err| err.into())
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_dir_subpath(
    &self,
    package_dir_path: &Path,
    package_subpath: &str,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageSubpathResolveError> {
    let package_json_path = package_dir_path.join("package.json");
    match self.load_package_json(&package_json_path)? {
      Some(pkg_json) => self.resolve_package_subpath(
        &pkg_json,
        package_subpath,
        maybe_referrer,
        referrer_kind,
        conditions,
        mode,
      ),
      None => self
        .resolve_package_subpath_no_pkg_json(
          package_dir_path,
          package_subpath,
          maybe_referrer,
          referrer_kind,
          mode,
        )
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
    referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Url, PackageSubpathResolveError> {
    if let Some(exports) = &package_json.exports {
      let result = self.package_exports_resolve(
        &package_json.path,
        package_subpath,
        exports,
        referrer,
        referrer_kind,
        conditions,
        mode,
      );
      match result {
        Ok(found) => return Ok(found),
        Err(exports_err) => {
          if mode.is_types() && package_subpath == "." {
            return self
              .legacy_main_resolve(package_json, referrer, referrer_kind, mode)
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
      return self
        .legacy_main_resolve(package_json, referrer, referrer_kind, mode)
        .map_err(|err| {
          PackageSubpathResolveErrorKind::LegacyResolve(err).into()
        });
    }

    self
      .resolve_subpath_exact(
        package_json.path.parent().unwrap(),
        package_subpath,
        referrer,
        referrer_kind,
        mode,
      )
      .map_err(|err| {
        PackageSubpathResolveErrorKind::LegacyResolve(err.into()).into()
      })
  }

  fn resolve_subpath_exact(
    &self,
    directory: &Path,
    package_subpath: &str,
    referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    mode: NodeResolutionMode,
  ) -> Result<Url, TypesNotFoundError> {
    assert_ne!(package_subpath, ".");
    let file_path = directory.join(package_subpath);
    if mode.is_types() {
      Ok(self.path_to_declaration_url(&file_path, referrer, referrer_kind)?)
    } else {
      Ok(url_from_file_path(&file_path).unwrap())
    }
  }

  fn resolve_package_subpath_no_pkg_json(
    &self,
    directory: &Path,
    package_subpath: &str,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    mode: NodeResolutionMode,
  ) -> Result<Url, LegacyResolveError> {
    if package_subpath == "." {
      self.legacy_index_resolve(directory, maybe_referrer, referrer_kind, mode)
    } else {
      self
        .resolve_subpath_exact(
          directory,
          package_subpath,
          maybe_referrer,
          referrer_kind,
          mode,
        )
        .map_err(|err| err.into())
    }
  }

  pub fn get_closest_package_json(
    &self,
    url: &Url,
  ) -> Result<Option<PackageJsonRc>, ClosestPkgJsonError> {
    let Ok(file_path) = deno_path_util::url_to_file_path(url) else {
      return Ok(None);
    };
    self.get_closest_package_json_from_path(&file_path)
  }

  pub fn get_closest_package_json_from_path(
    &self,
    file_path: &Path,
  ) -> Result<Option<PackageJsonRc>, ClosestPkgJsonError> {
    // we use this for deno compile using byonm because the script paths
    // won't be in virtual file system, but the package.json paths will be
    fn canonicalize_first_ancestor_exists(
      dir_path: &Path,
      env: &dyn NodeResolverEnv,
    ) -> Result<Option<PathBuf>, std::io::Error> {
      for ancestor in dir_path.ancestors() {
        match env.realpath_sync(ancestor) {
          Ok(dir_path) => return Ok(Some(dir_path)),
          Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // keep searching
          }
          Err(err) => return Err(err),
        }
      }
      Ok(None)
    }

    let parent_dir = file_path.parent().unwrap();
    let Some(start_dir) = canonicalize_first_ancestor_exists(
      parent_dir, &self.env,
    )
    .map_err(|source| CanonicalizingPkgJsonDirError {
      dir_path: parent_dir.to_path_buf(),
      source,
    })?
    else {
      return Ok(None);
    };
    let start_dir = strip_unc_prefix(start_dir);
    for current_dir in start_dir.ancestors() {
      let package_json_path = current_dir.join("package.json");
      if let Some(pkg_json) = self.load_package_json(&package_json_path)? {
        return Ok(Some(pkg_json));
      }
    }

    Ok(None)
  }

  pub fn load_package_json(
    &self,
    package_json_path: &Path,
  ) -> Result<Option<PackageJsonRc>, PackageJsonLoadError> {
    crate::package_json::load_pkg_json(
      self.env.pkg_json_fs(),
      package_json_path,
    )
  }

  pub(super) fn legacy_main_resolve(
    &self,
    package_json: &PackageJson,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    mode: NodeResolutionMode,
  ) -> Result<Url, LegacyResolveError> {
    let maybe_main = if mode.is_types() {
      match package_json.types.as_ref() {
        Some(types) => Some(types.as_str()),
        None => {
          // fallback to checking the main entrypoint for
          // a corresponding declaration file
          if let Some(main) = package_json.main(referrer_kind) {
            let main = package_json.path.parent().unwrap().join(main).clean();
            let decl_url_result = self.path_to_declaration_url(
              &main,
              maybe_referrer,
              referrer_kind,
            );
            // don't surface errors, fallback to checking the index now
            if let Ok(url) = decl_url_result {
              return Ok(url);
            }
          }
          None
        }
      }
    } else {
      package_json.main(referrer_kind)
    };

    if let Some(main) = maybe_main {
      let guess = package_json.path.parent().unwrap().join(main).clean();
      if self.env.is_file_sync(&guess) {
        return Ok(url_from_file_path(&guess).unwrap());
      }

      // todo(dsherret): investigate exactly how node and typescript handles this
      let endings = if mode.is_types() {
        match referrer_kind {
          NodeModuleKind::Cjs => {
            vec![".d.ts", ".d.cts", "/index.d.ts", "/index.d.cts"]
          }
          NodeModuleKind::Esm => vec![
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
        if self.env.is_file_sync(&guess) {
          // TODO(bartlomieju): emitLegacyIndexDeprecation()
          return Ok(url_from_file_path(&guess).unwrap());
        }
      }
    }

    self.legacy_index_resolve(
      package_json.path.parent().unwrap(),
      maybe_referrer,
      referrer_kind,
      mode,
    )
  }

  fn legacy_index_resolve(
    &self,
    directory: &Path,
    maybe_referrer: Option<&Url>,
    referrer_kind: NodeModuleKind,
    mode: NodeResolutionMode,
  ) -> Result<Url, LegacyResolveError> {
    let index_file_names = if mode.is_types() {
      // todo(dsherret): investigate exactly how typescript does this
      match referrer_kind {
        NodeModuleKind::Cjs => vec!["index.d.ts", "index.d.cts"],
        NodeModuleKind::Esm => vec!["index.d.ts", "index.d.mts", "index.d.cts"],
      }
    } else {
      vec!["index.js"]
    };
    for index_file_name in index_file_names {
      let guess = directory.join(index_file_name).clean();
      if self.env.is_file_sync(&guess) {
        // TODO(bartlomieju): emitLegacyIndexDeprecation()
        return Ok(url_from_file_path(&guess).unwrap());
      }
    }

    if mode.is_types() {
      Err(
        TypesNotFoundError(Box::new(TypesNotFoundErrorData {
          code_specifier: url_from_file_path(&directory.join("index.js"))
            .unwrap(),
          maybe_referrer: maybe_referrer.cloned(),
        }))
        .into(),
      )
    } else {
      Err(
        ModuleNotFoundError {
          specifier: url_from_file_path(&directory.join("index.js")).unwrap(),
          typ: "module",
          maybe_referrer: maybe_referrer.cloned(),
        }
        .into(),
      )
    }
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

fn to_file_path(url: &Url) -> PathBuf {
  deno_path_util::url_to_file_path(url).unwrap()
}

fn to_file_path_string(url: &Url) -> String {
  to_file_path(url).display().to_string()
}

fn should_be_treated_as_relative_or_absolute_path(specifier: &str) -> bool {
  if specifier.is_empty() {
    return false;
  }

  if specifier.starts_with('/') {
    return true;
  }

  is_relative_specifier(specifier)
}

// TODO(ry) We very likely have this utility function elsewhere in Deno.
fn is_relative_specifier(specifier: &str) -> bool {
  let specifier_len = specifier.len();
  let specifier_chars: Vec<_> = specifier.chars().take(3).collect();

  if !specifier_chars.is_empty() && specifier_chars[0] == '.' {
    if specifier_len == 1 || specifier_chars[1] == '/' {
      return true;
    }
    if specifier_chars[1] == '.'
      && (specifier_len == 2 || specifier_chars[2] == '/')
    {
      return true;
    }
  }
  false
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

fn to_specifier_display_string(url: &Url) -> String {
  if let Ok(path) = deno_path_util::url_to_file_path(url) {
    path.display().to_string()
  } else {
    url.to_string()
  }
}

fn throw_invalid_subpath(
  subpath: String,
  package_json_path: &Path,
  internal: bool,
  maybe_referrer: Option<&Url>,
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

pub fn parse_npm_pkg_name(
  specifier: &str,
  referrer: &Url,
) -> Result<(String, String, bool), InvalidModuleSpecifierError> {
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

  let package_name = if let Some(index) = separator_index {
    specifier[0..index].to_string()
  } else {
    specifier.to_string()
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

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
  } else {
    ".".to_string()
  };

  Ok((package_name, package_subpath, is_scoped))
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
fn types_package_name(package_name: &str) -> String {
  debug_assert!(!package_name.starts_with("@types/"));
  // Scoped packages will get two underscores for each slash
  // https://github.com/DefinitelyTyped/DefinitelyTyped/tree/15f1ece08f7b498f4b9a2147c2a46e94416ca777#what-about-scoped-packages
  format!("@types/{}", package_name.replace('/', "__"))
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

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  fn build_package_json(json: Value) -> PackageJson {
    PackageJson::load_from_value(PathBuf::from("/package.json"), json)
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

    assert_eq!(
      parse_npm_pkg_name("fetch-blob", &dummy_referrer).unwrap(),
      ("fetch-blob".to_string(), ".".to_string(), false)
    );
    assert_eq!(
      parse_npm_pkg_name("@vue/plugin-vue", &dummy_referrer).unwrap(),
      ("@vue/plugin-vue".to_string(), ".".to_string(), true)
    );
    assert_eq!(
      parse_npm_pkg_name("@astrojs/prism/dist/highlighter", &dummy_referrer)
        .unwrap(),
      (
        "@astrojs/prism".to_string(),
        "./dist/highlighter".to_string(),
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
      "@types/@scoped__package"
    );
  }
}
