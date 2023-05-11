// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_fs::FileSystemRc;
use deno_media_type::MediaType;
use deno_semver::npm::NpmPackageNv;
use deno_semver::npm::NpmPackageNvReference;
use deno_semver::npm::NpmPackageReqReference;

use crate::errors;
use crate::AllowAllNodePermissions;
use crate::NodePermissions;
use crate::NpmResolverRc;
use crate::PackageJson;
use crate::PathClean;

pub static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];
pub static REQUIRE_CONDITIONS: &[&str] = &["require", "node"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeModuleKind {
  Esm,
  Cjs,
}

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
  Esm(ModuleSpecifier),
  CommonJs(ModuleSpecifier),
  BuiltIn(String),
}

impl NodeResolution {
  pub fn into_url(self) -> ModuleSpecifier {
    match self {
      Self::Esm(u) => u,
      Self::CommonJs(u) => u,
      Self::BuiltIn(specifier) => {
        if specifier.starts_with("node:") {
          ModuleSpecifier::parse(&specifier).unwrap()
        } else {
          ModuleSpecifier::parse(&format!("node:{specifier}")).unwrap()
        }
      }
    }
  }

  pub fn into_specifier_and_media_type(
    resolution: Option<Self>,
  ) -> (ModuleSpecifier, MediaType) {
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
        ModuleSpecifier::parse("internal:///missing_dependency.d.ts").unwrap(),
        MediaType::Dts,
      ),
    }
  }
}

#[allow(clippy::disallowed_types)]
pub type NodeResolverRc = deno_fs::sync::MaybeArc<NodeResolver>;

#[derive(Debug)]
pub struct NodeResolver {
  fs: FileSystemRc,
  npm_resolver: NpmResolverRc,
}

impl NodeResolver {
  pub fn new(fs: FileSystemRc, npm_resolver: NpmResolverRc) -> Self {
    Self { fs, npm_resolver }
  }

  pub fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    self.npm_resolver.in_npm_package(specifier)
  }

  /// This function is an implementation of `defaultResolve` in
  /// `lib/internal/modules/esm/resolve.js` from Node.
  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<Option<NodeResolution>, AnyError> {
    // Note: if we are here, then the referrer is an esm module
    // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

    if crate::is_builtin_node_module(specifier) {
      return Ok(Some(NodeResolution::BuiltIn(specifier.to_string())));
    }

    if let Ok(url) = Url::parse(specifier) {
      if url.scheme() == "data" {
        return Ok(Some(NodeResolution::Esm(url)));
      }

      let protocol = url.scheme();

      if protocol == "node" {
        let split_specifier = url.as_str().split(':');
        let specifier = split_specifier.skip(1).collect::<String>();

        if crate::is_builtin_node_module(&specifier) {
          return Ok(Some(NodeResolution::BuiltIn(specifier)));
        }
      }

      if protocol != "file" && protocol != "data" {
        return Err(errors::err_unsupported_esm_url_scheme(&url));
      }

      // todo(dsherret): this seems wrong
      if referrer.scheme() == "data" {
        let url = referrer.join(specifier).map_err(AnyError::from)?;
        return Ok(Some(NodeResolution::Esm(url)));
      }
    }

    let url = self.module_resolve(
      specifier,
      referrer,
      DEFAULT_CONDITIONS,
      mode,
      permissions,
    )?;
    let url = match url {
      Some(url) => url,
      None => return Ok(None),
    };
    let url = match mode {
      NodeResolutionMode::Execution => url,
      NodeResolutionMode::Types => {
        let path = url.to_file_path().unwrap();
        // todo(16370): the module kind is not correct here. I think we need
        // typescript to tell us if the referrer is esm or cjs
        let path =
          match self.path_to_declaration_path(path, NodeModuleKind::Esm) {
            Some(path) => path,
            None => return Ok(None),
          };
        ModuleSpecifier::from_file_path(path).unwrap()
      }
    };

    let resolve_response = self.url_to_node_resolution(url)?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(Some(resolve_response))
  }

  fn module_resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<Option<ModuleSpecifier>, AnyError> {
    // note: if we're here, the referrer is an esm module
    let url = if should_be_treated_as_relative_or_absolute_path(specifier) {
      let resolved_specifier = referrer.join(specifier)?;
      if mode.is_types() {
        let file_path = to_file_path(&resolved_specifier);
        // todo(dsherret): the node module kind is not correct and we
        // should use the value provided by typescript instead
        let declaration_path =
          self.path_to_declaration_path(file_path, NodeModuleKind::Esm);
        declaration_path.map(|declaration_path| {
          ModuleSpecifier::from_file_path(declaration_path).unwrap()
        })
      } else {
        Some(resolved_specifier)
      }
    } else if specifier.starts_with('#') {
      Some(
        self
          .package_imports_resolve(
            specifier,
            referrer,
            NodeModuleKind::Esm,
            conditions,
            mode,
            permissions,
          )
          .map(|p| ModuleSpecifier::from_file_path(p).unwrap())?,
      )
    } else if let Ok(resolved) = Url::parse(specifier) {
      Some(resolved)
    } else {
      self
        .package_resolve(
          specifier,
          referrer,
          NodeModuleKind::Esm,
          conditions,
          mode,
          permissions,
        )?
        .map(|p| ModuleSpecifier::from_file_path(p).unwrap())
    };
    Ok(match url {
      Some(url) => Some(self.finalize_resolution(url, referrer)?),
      None => None,
    })
  }

  fn finalize_resolution(
    &self,
    resolved: ModuleSpecifier,
    base: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    let encoded_sep_re = lazy_regex::regex!(r"%2F|%2C");

    if encoded_sep_re.is_match(resolved.path()) {
      return Err(errors::err_invalid_module_specifier(
        resolved.path(),
        "must not include encoded \"/\" or \"\\\\\" characters",
        Some(to_file_path_string(base)),
      ));
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

    let (is_dir, is_file) = if let Ok(stats) = self.fs.stat_sync(Path::new(&p))
    {
      (stats.is_directory, stats.is_file)
    } else {
      (false, false)
    };
    if is_dir {
      return Err(errors::err_unsupported_dir_import(
        resolved.as_str(),
        base.as_str(),
      ));
    } else if !is_file {
      return Err(errors::err_module_not_found(
        resolved.as_str(),
        base.as_str(),
        "module",
      ));
    }

    Ok(resolved)
  }

  pub fn resolve_npm_req_reference(
    &self,
    reference: &NpmPackageReqReference,
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<Option<NodeResolution>, AnyError> {
    let reference = self
      .npm_resolver
      .resolve_nv_ref_from_pkg_req_ref(reference)?;
    self.resolve_npm_reference(&reference, mode, permissions)
  }

  pub fn resolve_npm_reference(
    &self,
    reference: &NpmPackageNvReference,
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<Option<NodeResolution>, AnyError> {
    let package_folder = self
      .npm_resolver
      .resolve_package_folder_from_deno_module(&reference.nv)?;
    let node_module_kind = NodeModuleKind::Esm;
    let maybe_resolved_path = self
      .package_config_resolve(
        &reference
          .sub_path
          .as_ref()
          .map(|s| format!("./{s}"))
          .unwrap_or_else(|| ".".to_string()),
        &package_folder,
        node_module_kind,
        DEFAULT_CONDITIONS,
        mode,
        permissions,
      )
      .with_context(|| {
        format!("Error resolving package config for '{reference}'")
      })?;
    let resolved_path = match maybe_resolved_path {
      Some(resolved_path) => resolved_path,
      None => return Ok(None),
    };
    let resolved_path = match mode {
      NodeResolutionMode::Execution => resolved_path,
      NodeResolutionMode::Types => {
        match self.path_to_declaration_path(resolved_path, node_module_kind) {
          Some(path) => path,
          None => return Ok(None),
        }
      }
    };
    let url = ModuleSpecifier::from_file_path(resolved_path).unwrap();
    let resolve_response = self.url_to_node_resolution(url)?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(Some(resolve_response))
  }

  pub fn resolve_binary_commands(
    &self,
    pkg_nv: &NpmPackageNv,
  ) -> Result<Vec<String>, AnyError> {
    let package_folder = self
      .npm_resolver
      .resolve_package_folder_from_deno_module(pkg_nv)?;
    let package_json_path = package_folder.join("package.json");
    let package_json =
      self.load_package_json(&AllowAllNodePermissions, package_json_path)?;

    Ok(match package_json.bin {
      Some(Value::String(_)) => vec![pkg_nv.name.to_string()],
      Some(Value::Object(o)) => {
        o.into_iter().map(|(key, _)| key).collect::<Vec<_>>()
      }
      _ => Vec::new(),
    })
  }

  pub fn resolve_binary_export(
    &self,
    pkg_ref: &NpmPackageReqReference,
  ) -> Result<NodeResolution, AnyError> {
    let pkg_nv = self
      .npm_resolver
      .resolve_pkg_id_from_pkg_req(&pkg_ref.req)?
      .nv;
    let bin_name = pkg_ref.sub_path.as_deref();
    let package_folder = self
      .npm_resolver
      .resolve_package_folder_from_deno_module(&pkg_nv)?;
    let package_json_path = package_folder.join("package.json");
    let package_json =
      self.load_package_json(&AllowAllNodePermissions, package_json_path)?;
    let bin = match &package_json.bin {
      Some(bin) => bin,
      None => bail!(
        "package '{}' did not have a bin property in its package.json",
        &pkg_nv.name,
      ),
    };
    let bin_entry = resolve_bin_entry_value(&pkg_nv, bin_name, bin)?;
    let url =
      ModuleSpecifier::from_file_path(package_folder.join(bin_entry)).unwrap();

    let resolve_response = self.url_to_node_resolution(url)?;
    // TODO(bartlomieju): skipped checking errors for commonJS resolution and
    // "preserveSymlinksMain"/"preserveSymlinks" options.
    Ok(resolve_response)
  }

  pub fn url_to_node_resolution(
    &self,
    url: ModuleSpecifier,
  ) -> Result<NodeResolution, AnyError> {
    let url_str = url.as_str().to_lowercase();
    if url_str.starts_with("http") {
      Ok(NodeResolution::Esm(url))
    } else if url_str.ends_with(".js") || url_str.ends_with(".d.ts") {
      let package_config =
        self.get_closest_package_json(&url, &AllowAllNodePermissions)?;
      if package_config.typ == "module" {
        Ok(NodeResolution::Esm(url))
      } else {
        Ok(NodeResolution::CommonJs(url))
      }
    } else if url_str.ends_with(".mjs") || url_str.ends_with(".d.mts") {
      Ok(NodeResolution::Esm(url))
    } else if url_str.ends_with(".ts") {
      Err(generic_error(format!(
        "TypeScript files are not supported in npm packages: {url}"
      )))
    } else {
      Ok(NodeResolution::CommonJs(url))
    }
  }

  fn package_config_resolve(
    &self,
    package_subpath: &str,
    package_dir: &Path,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<Option<PathBuf>, AnyError> {
    let package_json_path = package_dir.join("package.json");
    let referrer = ModuleSpecifier::from_directory_path(package_dir).unwrap();
    let package_config =
      self.load_package_json(permissions, package_json_path.clone())?;
    if let Some(exports) = &package_config.exports {
      let result = self.package_exports_resolve(
        &package_json_path,
        package_subpath.to_string(),
        exports,
        &referrer,
        referrer_kind,
        conditions,
        mode,
        permissions,
      );
      match result {
        Ok(found) => return Ok(Some(found)),
        Err(exports_err) => {
          if mode.is_types() && package_subpath == "." {
            if let Ok(Some(path)) =
              self.legacy_main_resolve(&package_config, referrer_kind, mode)
            {
              return Ok(Some(path));
            } else {
              return Ok(None);
            }
          }
          return Err(exports_err);
        }
      }
    }
    if package_subpath == "." {
      return self.legacy_main_resolve(&package_config, referrer_kind, mode);
    }

    Ok(Some(package_dir.join(package_subpath)))
  }

  /// Checks if the resolved file has a corresponding declaration file.
  pub(super) fn path_to_declaration_path(
    &self,
    path: PathBuf,
    referrer_kind: NodeModuleKind,
  ) -> Option<PathBuf> {
    fn probe_extensions(
      fs: &dyn deno_fs::FileSystem,
      path: &Path,
      referrer_kind: NodeModuleKind,
    ) -> Option<PathBuf> {
      let specific_dts_path = match referrer_kind {
        NodeModuleKind::Cjs => with_known_extension(path, "d.cts"),
        NodeModuleKind::Esm => with_known_extension(path, "d.mts"),
      };
      if fs.exists(&specific_dts_path) {
        return Some(specific_dts_path);
      }
      let dts_path = with_known_extension(path, "d.ts");
      if fs.exists(&dts_path) {
        Some(dts_path)
      } else {
        None
      }
    }

    let lowercase_path = path.to_string_lossy().to_lowercase();
    if lowercase_path.ends_with(".d.ts")
      || lowercase_path.ends_with(".d.cts")
      || lowercase_path.ends_with(".d.ts")
    {
      return Some(path);
    }
    if let Some(path) = probe_extensions(&*self.fs, &path, referrer_kind) {
      return Some(path);
    }
    if self.fs.is_dir(&path) {
      if let Some(path) =
        probe_extensions(&*self.fs, &path.join("index"), referrer_kind)
      {
        return Some(path);
      }
    }
    None
  }

  pub(super) fn package_imports_resolve(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<PathBuf, AnyError> {
    if name == "#" || name.starts_with("#/") || name.ends_with('/') {
      let reason = "is not a valid internal imports specifier name";
      return Err(errors::err_invalid_module_specifier(
        name,
        reason,
        Some(to_specifier_display_string(referrer)),
      ));
    }

    let package_config =
      self.get_package_scope_config(referrer, permissions)?;
    let mut package_json_path = None;
    if package_config.exists {
      package_json_path = Some(package_config.path.clone());
      if let Some(imports) = &package_config.imports {
        if imports.contains_key(name) && !name.contains('*') {
          let maybe_resolved = self.resolve_package_target(
            package_json_path.as_ref().unwrap(),
            imports.get(name).unwrap().to_owned(),
            "".to_string(),
            name.to_string(),
            referrer,
            referrer_kind,
            false,
            true,
            conditions,
            mode,
            permissions,
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
              let key_sub = &key[0..=pattern_index];
              if name.starts_with(key_sub) {
                let pattern_trailer = &key[pattern_index + 1..];
                if name.len() > key.len()
                  && name.ends_with(&pattern_trailer)
                  && pattern_key_compare(best_match, key) == 1
                  && key.rfind('*') == Some(pattern_index)
                {
                  best_match = key;
                  best_match_subpath = Some(
                    name[pattern_index..=(name.len() - pattern_trailer.len())]
                      .to_string(),
                  );
                }
              }
            }
          }

          if !best_match.is_empty() {
            let target = imports.get(best_match).unwrap().to_owned();
            let maybe_resolved = self.resolve_package_target(
              package_json_path.as_ref().unwrap(),
              target,
              best_match_subpath.unwrap(),
              best_match.to_string(),
              referrer,
              referrer_kind,
              true,
              true,
              conditions,
              mode,
              permissions,
            )?;
            if let Some(resolved) = maybe_resolved {
              return Ok(resolved);
            }
          }
        }
      }
    }

    Err(throw_import_not_defined(
      name,
      package_json_path.as_deref(),
      referrer,
    ))
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_target_string(
    &self,
    target: String,
    subpath: String,
    match_: String,
    package_json_path: &Path,
    referrer: &ModuleSpecifier,
    referrer_kind: NodeModuleKind,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<PathBuf, AnyError> {
    if !subpath.is_empty() && !pattern && !target.ends_with('/') {
      return Err(throw_invalid_package_target(
        match_,
        target,
        package_json_path,
        internal,
        referrer,
      ));
    }
    let invalid_segment_re =
      lazy_regex::regex!(r"(^|\\|/)(\.\.?|node_modules)(\\|/|$)");
    let pattern_re = lazy_regex::regex!(r"\*");
    if !target.starts_with("./") {
      if internal && !target.starts_with("../") && !target.starts_with('/') {
        let is_url = Url::parse(&target).is_ok();
        if !is_url {
          let export_target = if pattern {
            pattern_re
              .replace(&target, |_caps: &regex::Captures| subpath.clone())
              .to_string()
          } else {
            format!("{target}{subpath}")
          };
          let package_json_url =
            ModuleSpecifier::from_file_path(package_json_path).unwrap();
          return match self.package_resolve(
            &export_target,
            &package_json_url,
            referrer_kind,
            conditions,
            mode,
            permissions,
          ) {
            Ok(Some(path)) => Ok(path),
            Ok(None) => Err(generic_error("not found")),
            Err(err) => Err(err),
          };
        }
      }
      return Err(throw_invalid_package_target(
        match_,
        target,
        package_json_path,
        internal,
        referrer,
      ));
    }
    if invalid_segment_re.is_match(&target[2..]) {
      return Err(throw_invalid_package_target(
        match_,
        target,
        package_json_path,
        internal,
        referrer,
      ));
    }
    let package_path = package_json_path.parent().unwrap();
    let resolved_path = package_path.join(&target).clean();
    if !resolved_path.starts_with(package_path) {
      return Err(throw_invalid_package_target(
        match_,
        target,
        package_json_path,
        internal,
        referrer,
      ));
    }
    if subpath.is_empty() {
      return Ok(resolved_path);
    }
    if invalid_segment_re.is_match(&subpath) {
      let request = if pattern {
        match_.replace('*', &subpath)
      } else {
        format!("{match_}{subpath}")
      };
      return Err(throw_invalid_subpath(
        request,
        package_json_path,
        internal,
        referrer,
      ));
    }
    if pattern {
      let resolved_path_str = resolved_path.to_string_lossy();
      let replaced = pattern_re
        .replace(&resolved_path_str, |_caps: &regex::Captures| {
          subpath.clone()
        });
      return Ok(PathBuf::from(replaced.to_string()));
    }
    Ok(resolved_path.join(&subpath).clean())
  }

  #[allow(clippy::too_many_arguments)]
  fn resolve_package_target(
    &self,
    package_json_path: &Path,
    target: Value,
    subpath: String,
    package_subpath: String,
    referrer: &ModuleSpecifier,
    referrer_kind: NodeModuleKind,
    pattern: bool,
    internal: bool,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<Option<PathBuf>, AnyError> {
    if let Some(target) = target.as_str() {
      return self
        .resolve_package_target_string(
          target.to_string(),
          subpath,
          package_subpath,
          package_json_path,
          referrer,
          referrer_kind,
          pattern,
          internal,
          conditions,
          mode,
          permissions,
        )
        .map(|path| {
          if mode.is_types() {
            self.path_to_declaration_path(path, referrer_kind)
          } else {
            Some(path)
          }
        });
    } else if let Some(target_arr) = target.as_array() {
      if target_arr.is_empty() {
        return Ok(None);
      }

      let mut last_error = None;
      for target_item in target_arr {
        let resolved_result = self.resolve_package_target(
          package_json_path,
          target_item.to_owned(),
          subpath.clone(),
          package_subpath.clone(),
          referrer,
          referrer_kind,
          pattern,
          internal,
          conditions,
          mode,
          permissions,
        );

        match resolved_result {
          Ok(Some(resolved)) => return Ok(Some(resolved)),
          Ok(None) => {
            last_error = None;
            continue;
          }
          Err(e) => {
            let err_string = e.to_string();
            last_error = Some(e);
            if err_string.starts_with("[ERR_INVALID_PACKAGE_TARGET]") {
              continue;
            }
            return Err(last_error.unwrap());
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
          let condition_target = target_obj.get(key).unwrap().to_owned();

          let resolved = self.resolve_package_target(
            package_json_path,
            condition_target,
            subpath.clone(),
            package_subpath.clone(),
            referrer,
            referrer_kind,
            pattern,
            internal,
            conditions,
            mode,
            permissions,
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

    Err(throw_invalid_package_target(
      package_subpath,
      target.to_string(),
      package_json_path,
      internal,
      referrer,
    ))
  }

  #[allow(clippy::too_many_arguments)]
  pub fn package_exports_resolve(
    &self,
    package_json_path: &Path,
    package_subpath: String,
    package_exports: &Map<String, Value>,
    referrer: &ModuleSpecifier,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<PathBuf, AnyError> {
    if package_exports.contains_key(&package_subpath)
      && package_subpath.find('*').is_none()
      && !package_subpath.ends_with('/')
    {
      let target = package_exports.get(&package_subpath).unwrap().to_owned();
      let resolved = self.resolve_package_target(
        package_json_path,
        target,
        "".to_string(),
        package_subpath.to_string(),
        referrer,
        referrer_kind,
        false,
        false,
        conditions,
        mode,
        permissions,
      )?;
      if resolved.is_none() {
        return Err(throw_exports_not_found(
          package_subpath,
          package_json_path,
          referrer,
        ));
      }
      return Ok(resolved.unwrap());
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
      let target = package_exports.get(best_match).unwrap().to_owned();
      let maybe_resolved = self.resolve_package_target(
        package_json_path,
        target,
        best_match_subpath.unwrap(),
        best_match.to_string(),
        referrer,
        referrer_kind,
        true,
        false,
        conditions,
        mode,
        permissions,
      )?;
      if let Some(resolved) = maybe_resolved {
        return Ok(resolved);
      } else {
        return Err(throw_exports_not_found(
          package_subpath,
          package_json_path,
          referrer,
        ));
      }
    }

    Err(throw_exports_not_found(
      package_subpath,
      package_json_path,
      referrer,
    ))
  }

  pub(super) fn package_resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    referrer_kind: NodeModuleKind,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<Option<PathBuf>, AnyError> {
    let (package_name, package_subpath, _is_scoped) =
      parse_package_name(specifier, referrer)?;

    // ResolveSelf
    let package_config =
      self.get_package_scope_config(referrer, permissions)?;
    if package_config.exists
      && package_config.name.as_ref() == Some(&package_name)
    {
      if let Some(exports) = &package_config.exports {
        return self
          .package_exports_resolve(
            &package_config.path,
            package_subpath,
            exports,
            referrer,
            referrer_kind,
            conditions,
            mode,
            permissions,
          )
          .map(Some);
      }
    }

    let package_dir_path = self
      .npm_resolver
      .resolve_package_folder_from_package(&package_name, referrer, mode)?;
    let package_json_path = package_dir_path.join("package.json");

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
    let package_json =
      self.load_package_json(permissions, package_json_path)?;
    if let Some(exports) = &package_json.exports {
      return self
        .package_exports_resolve(
          &package_json.path,
          package_subpath,
          exports,
          referrer,
          referrer_kind,
          conditions,
          mode,
          permissions,
        )
        .map(Some);
    }
    if package_subpath == "." {
      return self.legacy_main_resolve(&package_json, referrer_kind, mode);
    }

    let file_path = package_json.path.parent().unwrap().join(&package_subpath);

    if mode.is_types() {
      let maybe_declaration_path =
        self.path_to_declaration_path(file_path, referrer_kind);
      Ok(maybe_declaration_path)
    } else {
      Ok(Some(file_path))
    }
  }

  pub(super) fn get_package_scope_config(
    &self,
    referrer: &ModuleSpecifier,
    permissions: &dyn NodePermissions,
  ) -> Result<PackageJson, AnyError> {
    let root_folder = self
      .npm_resolver
      .resolve_package_folder_from_path(&referrer.to_file_path().unwrap())?;
    let package_json_path = root_folder.join("package.json");
    self.load_package_json(permissions, package_json_path)
  }

  pub(super) fn get_closest_package_json(
    &self,
    url: &ModuleSpecifier,
    permissions: &dyn NodePermissions,
  ) -> Result<PackageJson, AnyError> {
    let package_json_path = self.get_closest_package_json_path(url)?;
    self.load_package_json(permissions, package_json_path)
  }

  fn get_closest_package_json_path(
    &self,
    url: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    let file_path = url.to_file_path().unwrap();
    let current_dir = deno_core::strip_unc_prefix(
      self.fs.realpath_sync(file_path.parent().unwrap())?,
    );
    let mut current_dir = current_dir.as_path();
    let package_json_path = current_dir.join("package.json");
    if self.fs.exists(&package_json_path) {
      return Ok(package_json_path);
    }
    let root_pkg_folder = self
      .npm_resolver
      .resolve_package_folder_from_path(current_dir)?;
    while current_dir.starts_with(&root_pkg_folder) {
      current_dir = current_dir.parent().unwrap();
      let package_json_path = current_dir.join("package.json");
      if self.fs.exists(&package_json_path) {
        return Ok(package_json_path);
      }
    }

    bail!("did not find package.json in {}", root_pkg_folder.display())
  }

  pub(super) fn load_package_json(
    &self,
    permissions: &dyn NodePermissions,
    package_json_path: PathBuf,
  ) -> Result<PackageJson, AnyError> {
    PackageJson::load(
      &*self.fs,
      &*self.npm_resolver,
      permissions,
      package_json_path,
    )
  }

  pub(super) fn legacy_main_resolve(
    &self,
    package_json: &PackageJson,
    referrer_kind: NodeModuleKind,
    mode: NodeResolutionMode,
  ) -> Result<Option<PathBuf>, AnyError> {
    let maybe_main = if mode.is_types() {
      match package_json.types.as_ref() {
        Some(types) => Some(types),
        None => {
          // fallback to checking the main entrypoint for
          // a corresponding declaration file
          if let Some(main) = package_json.main(referrer_kind) {
            let main = package_json.path.parent().unwrap().join(main).clean();
            if let Some(path) =
              self.path_to_declaration_path(main, referrer_kind)
            {
              return Ok(Some(path));
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
      if self.fs.is_file(&guess) {
        return Ok(Some(guess));
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
        if self.fs.is_file(&guess) {
          // TODO(bartlomieju): emitLegacyIndexDeprecation()
          return Ok(Some(guess));
        }
      }
    }

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
      let guess = package_json
        .path
        .parent()
        .unwrap()
        .join(index_file_name)
        .clean();
      if self.fs.is_file(&guess) {
        // TODO(bartlomieju): emitLegacyIndexDeprecation()
        return Ok(Some(guess));
      }
    }

    Ok(None)
  }
}

fn resolve_bin_entry_value<'a>(
  pkg_nv: &NpmPackageNv,
  bin_name: Option<&str>,
  bin: &'a Value,
) -> Result<&'a str, AnyError> {
  let bin_entry = match bin {
    Value::String(_) => {
      if bin_name.is_some() && bin_name.unwrap() != pkg_nv.name {
        None
      } else {
        Some(bin)
      }
    }
    Value::Object(o) => {
      if let Some(bin_name) = bin_name {
        o.get(bin_name)
      } else if o.len() == 1 || o.len() > 1 && o.values().all(|v| v == o.values().next().unwrap()) {
        o.values().next()
      } else {
        o.get(&pkg_nv.name)
      }
    },
    _ => bail!("package '{}' did not have a bin property with a string or object value in its package.json", pkg_nv),
  };
  let bin_entry = match bin_entry {
    Some(e) => e,
    None => {
      let keys = bin
        .as_object()
        .map(|o| {
          o.keys()
            .map(|k| format!(" * npm:{pkg_nv}/{k}"))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
      bail!(
        "package '{}' did not have a bin entry for '{}' in its package.json{}",
        pkg_nv,
        bin_name.unwrap_or(&pkg_nv.name),
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
      "package '{}' had a non-string sub property of bin in its package.json",
      pkg_nv,
    ),
  }
}

fn to_file_path(url: &ModuleSpecifier) -> PathBuf {
  url
    .to_file_path()
    .unwrap_or_else(|_| panic!("Provided URL was not file:// URL: {url}"))
}

fn to_file_path_string(url: &ModuleSpecifier) -> String {
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
  let specifier_chars: Vec<_> = specifier.chars().collect();

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
  const NON_DECL_EXTS: &[&str] = &["cjs", "js", "json", "jsx", "mjs", "tsx"];
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

fn to_specifier_display_string(url: &ModuleSpecifier) -> String {
  if let Ok(path) = url.to_file_path() {
    path.display().to_string()
  } else {
    url.to_string()
  }
}

fn throw_import_not_defined(
  specifier: &str,
  package_json_path: Option<&Path>,
  base: &ModuleSpecifier,
) -> AnyError {
  errors::err_package_import_not_defined(
    specifier,
    package_json_path.map(|p| p.parent().unwrap().display().to_string()),
    &to_specifier_display_string(base),
  )
}

fn throw_invalid_package_target(
  subpath: String,
  target: String,
  package_json_path: &Path,
  internal: bool,
  referrer: &ModuleSpecifier,
) -> AnyError {
  errors::err_invalid_package_target(
    package_json_path.parent().unwrap().display().to_string(),
    subpath,
    target,
    internal,
    Some(referrer.to_string()),
  )
}

fn throw_invalid_subpath(
  subpath: String,
  package_json_path: &Path,
  internal: bool,
  referrer: &ModuleSpecifier,
) -> AnyError {
  let ie = if internal { "imports" } else { "exports" };
  let reason = format!(
    "request is not a valid subpath for the \"{}\" resolution of {}",
    ie,
    package_json_path.display(),
  );
  errors::err_invalid_module_specifier(
    &subpath,
    &reason,
    Some(to_specifier_display_string(referrer)),
  )
}

fn throw_exports_not_found(
  subpath: String,
  package_json_path: &Path,
  referrer: &ModuleSpecifier,
) -> AnyError {
  errors::err_package_path_not_exported(
    package_json_path.parent().unwrap().display().to_string(),
    subpath,
    Some(to_specifier_display_string(referrer)),
  )
}

fn parse_package_name(
  specifier: &str,
  referrer: &ModuleSpecifier,
) -> Result<(String, String, bool), AnyError> {
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
    return Err(errors::err_invalid_module_specifier(
      specifier,
      "is not a valid package name",
      Some(to_specifier_display_string(referrer)),
    ));
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

#[cfg(test)]
mod tests {
  use deno_core::serde_json::json;

  use super::*;

  #[test]
  fn test_resolve_bin_entry_value() {
    // should resolve the specified value
    let value = json!({
      "bin1": "./value1",
      "bin2": "./value2",
      "test": "./value3",
    });
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageNv::from_str("test@1.1.1").unwrap(),
        Some("bin1"),
        &value
      )
      .unwrap(),
      "./value1"
    );

    // should resolve the value with the same name when not specified
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageNv::from_str("test@1.1.1").unwrap(),
        None,
        &value
      )
      .unwrap(),
      "./value3"
    );

    // should not resolve when specified value does not exist
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageNv::from_str("test@1.1.1").unwrap(),
        Some("other"),
        &value
      )
      .err()
      .unwrap()
      .to_string(),
      concat!(
        "package 'test@1.1.1' did not have a bin entry for 'other' in its package.json\n",
        "\n",
        "Possibilities:\n",
        " * npm:test@1.1.1/bin1\n",
        " * npm:test@1.1.1/bin2\n",
        " * npm:test@1.1.1/test"
      )
    );

    // should not resolve when default value can't be determined
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageNv::from_str("asdf@1.2.3").unwrap(),
        None,
        &value
      )
      .err()
      .unwrap()
      .to_string(),
      concat!(
        "package 'asdf@1.2.3' did not have a bin entry for 'asdf' in its package.json\n",
        "\n",
        "Possibilities:\n",
        " * npm:asdf@1.2.3/bin1\n",
        " * npm:asdf@1.2.3/bin2\n",
        " * npm:asdf@1.2.3/test"
      )
    );

    // should resolve since all the values are the same
    let value = json!({
      "bin1": "./value",
      "bin2": "./value",
    });
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageNv::from_str("test@1.2.3").unwrap(),
        None,
        &value
      )
      .unwrap(),
      "./value"
    );

    // should not resolve when specified and is a string
    let value = json!("./value");
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageNv::from_str("test@1.2.3").unwrap(),
        Some("path"),
        &value
      )
      .err()
      .unwrap()
      .to_string(),
      "package 'test@1.2.3' did not have a bin entry for 'path' in its package.json"
    );
  }

  #[test]
  fn test_parse_package_name() {
    let dummy_referrer = Url::parse("http://example.com").unwrap();

    assert_eq!(
      parse_package_name("fetch-blob", &dummy_referrer).unwrap(),
      ("fetch-blob".to_string(), ".".to_string(), false)
    );
    assert_eq!(
      parse_package_name("@vue/plugin-vue", &dummy_referrer).unwrap(),
      ("@vue/plugin-vue".to_string(), ".".to_string(), true)
    );
    assert_eq!(
      parse_package_name("@astrojs/prism/dist/highlighter", &dummy_referrer)
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
}
