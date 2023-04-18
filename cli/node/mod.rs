// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_runtime::deno_node;
use deno_runtime::deno_node::errors;
use deno_runtime::deno_node::find_builtin_node_module;
use deno_runtime::deno_node::get_closest_package_json;
use deno_runtime::deno_node::legacy_main_resolve;
use deno_runtime::deno_node::package_exports_resolve;
use deno_runtime::deno_node::package_imports_resolve;
use deno_runtime::deno_node::package_resolve;
use deno_runtime::deno_node::path_to_declaration_path;
use deno_runtime::deno_node::NodeModuleKind;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_node::RealFs;
use deno_runtime::deno_node::RequireNpmResolver;
use deno_runtime::deno_node::DEFAULT_CONDITIONS;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageNv;
use deno_semver::npm::NpmPackageNvReference;
use deno_semver::npm::NpmPackageReqReference;

use crate::npm::NpmPackageResolver;
use crate::npm::NpmResolution;
use crate::npm::RequireNpmPackageResolver;
use crate::util::fs::canonicalize_path_maybe_not_exists;

mod analyze;

pub use analyze::NodeCodeTranslator;

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

// TODO(bartlomieju): seems super wasteful to parse specified each time
pub fn resolve_builtin_node_module(module_name: &str) -> Result<Url, AnyError> {
  if let Some(module) = find_builtin_node_module(module_name) {
    return Ok(ModuleSpecifier::parse(module.specifier).unwrap());
  }

  Err(generic_error(format!(
    "Unknown built-in \"node:\" module: {module_name}"
  )))
}

#[derive(Debug)]
pub struct CliNodeResolver {
  npm_resolution: Arc<NpmResolution>,
  npm_resolver: Arc<NpmPackageResolver>,
  require_npm_resolver: RequireNpmPackageResolver,
}

impl CliNodeResolver {
  pub fn new(
    npm_resolution: Arc<NpmResolution>,
    npm_package_resolver: Arc<NpmPackageResolver>,
  ) -> Self {
    Self {
      npm_resolution,
      require_npm_resolver: npm_package_resolver.as_require_npm_resolver(),
      npm_resolver: npm_package_resolver,
    }
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
    permissions: &mut dyn NodePermissions,
  ) -> Result<Option<NodeResolution>, AnyError> {
    // Note: if we are here, then the referrer is an esm module
    // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

    if deno_node::is_builtin_node_module(specifier) {
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

        if deno_node::is_builtin_node_module(&specifier) {
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
          match path_to_declaration_path::<RealFs>(path, NodeModuleKind::Esm) {
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
    permissions: &mut dyn NodePermissions,
  ) -> Result<Option<ModuleSpecifier>, AnyError> {
    // note: if we're here, the referrer is an esm module
    let url = if should_be_treated_as_relative_or_absolute_path(specifier) {
      let resolved_specifier = referrer.join(specifier)?;
      if mode.is_types() {
        let file_path = to_file_path(&resolved_specifier);
        // todo(dsherret): the node module kind is not correct and we
        // should use the value provided by typescript instead
        let declaration_path =
          path_to_declaration_path::<RealFs>(file_path, NodeModuleKind::Esm);
        declaration_path.map(|declaration_path| {
          ModuleSpecifier::from_file_path(declaration_path).unwrap()
        })
      } else {
        Some(resolved_specifier)
      }
    } else if specifier.starts_with('#') {
      Some(
        package_imports_resolve::<RealFs>(
          specifier,
          referrer,
          NodeModuleKind::Esm,
          conditions,
          mode,
          &self.require_npm_resolver,
          permissions,
        )
        .map(|p| ModuleSpecifier::from_file_path(p).unwrap())?,
      )
    } else if let Ok(resolved) = Url::parse(specifier) {
      Some(resolved)
    } else {
      package_resolve::<RealFs>(
        specifier,
        referrer,
        NodeModuleKind::Esm,
        conditions,
        mode,
        &self.require_npm_resolver,
        permissions,
      )?
      .map(|p| ModuleSpecifier::from_file_path(p).unwrap())
    };
    Ok(match url {
      Some(url) => Some(finalize_resolution(url, referrer)?),
      None => None,
    })
  }

  pub fn resolve_npm_req_reference(
    &self,
    reference: &NpmPackageReqReference,
    mode: NodeResolutionMode,
    permissions: &mut dyn NodePermissions,
  ) -> Result<Option<NodeResolution>, AnyError> {
    let reference = self.npm_resolution.pkg_req_ref_to_nv_ref(reference)?;
    self.resolve_npm_reference(&reference, mode, permissions)
  }

  pub fn resolve_npm_reference(
    &self,
    reference: &NpmPackageNvReference,
    mode: NodeResolutionMode,
    permissions: &mut dyn NodePermissions,
  ) -> Result<Option<NodeResolution>, AnyError> {
    let package_folder = self
      .npm_resolver
      .resolve_package_folder_from_deno_module(&reference.nv)?;
    let node_module_kind = NodeModuleKind::Esm;
    let maybe_resolved_path = package_config_resolve(
      &reference
        .sub_path
        .as_ref()
        .map(|s| format!("./{s}"))
        .unwrap_or_else(|| ".".to_string()),
      &package_folder,
      node_module_kind,
      DEFAULT_CONDITIONS,
      mode,
      &self.require_npm_resolver,
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
        match path_to_declaration_path::<RealFs>(
          resolved_path,
          node_module_kind,
        ) {
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
    let package_json = PackageJson::load::<RealFs>(
      &self.require_npm_resolver,
      &mut PermissionsContainer::allow_all(),
      package_json_path,
    )?;

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
      .npm_resolution
      .resolve_pkg_id_from_pkg_req(&pkg_ref.req)?
      .nv;
    let bin_name = pkg_ref.sub_path.as_deref();
    let package_folder = self
      .npm_resolver
      .resolve_package_folder_from_deno_module(&pkg_nv)?;
    let package_json_path = package_folder.join("package.json");
    let package_json = PackageJson::load::<RealFs>(
      &self.require_npm_resolver,
      &mut PermissionsContainer::allow_all(),
      package_json_path,
    )?;
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
      let package_config = get_closest_package_json::<RealFs>(
        &url,
        &self.require_npm_resolver,
        &mut PermissionsContainer::allow_all(),
      )?;
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
}

/// Resolves a specifier that is pointing into a node_modules folder.
///
/// Note: This should be called whenever getting the specifier from
/// a Module::External(module) reference because that module might
/// not be fully resolved at the time deno_graph is analyzing it
/// because the node_modules folder might not exist at that time.
pub fn resolve_specifier_into_node_modules(
  specifier: &ModuleSpecifier,
) -> ModuleSpecifier {
  specifier
    .to_file_path()
    .ok()
    // this path might not exist at the time the graph is being created
    // because the node_modules folder might not yet exist
    .and_then(|path| canonicalize_path_maybe_not_exists(&path).ok())
    .and_then(|path| ModuleSpecifier::from_file_path(path).ok())
    .unwrap_or_else(|| specifier.clone())
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

fn package_config_resolve(
  package_subpath: &str,
  package_dir: &Path,
  referrer_kind: NodeModuleKind,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<Option<PathBuf>, AnyError> {
  let package_json_path = package_dir.join("package.json");
  let referrer = ModuleSpecifier::from_directory_path(package_dir).unwrap();
  let package_config = PackageJson::load::<RealFs>(
    npm_resolver,
    permissions,
    package_json_path.clone(),
  )?;
  if let Some(exports) = &package_config.exports {
    let result = package_exports_resolve::<RealFs>(
      &package_json_path,
      package_subpath.to_string(),
      exports,
      &referrer,
      referrer_kind,
      conditions,
      mode,
      npm_resolver,
      permissions,
    );
    match result {
      Ok(found) => return Ok(Some(found)),
      Err(exports_err) => {
        if mode.is_types() && package_subpath == "." {
          if let Ok(Some(path)) =
            legacy_main_resolve::<RealFs>(&package_config, referrer_kind, mode)
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
    return legacy_main_resolve::<RealFs>(&package_config, referrer_kind, mode);
  }

  Ok(Some(package_dir.join(package_subpath)))
}

fn finalize_resolution(
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

  let (is_dir, is_file) = if let Ok(stats) = std::fs::metadata(p) {
    (stats.is_dir(), stats.is_file())
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
}
