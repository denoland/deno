// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use regex::Regex;

/// This function is an implementation of `defaultResolve` in
/// `lib/internal/modules/esm/resolve.js` from Node.
// fn node_resolve(
//   specifier: &str,
//   referrer: &str,
//   is_main: bool,
// ) -> Result<ModuleSpecifier, AnyError> {
//   // TODO(bartlomieju): shipped "policy" part

//   if let Ok(url) = Url::parse(specifier) {
//     if url.scheme() == "data:" {
//       return Ok(url);
//     }

//     let protocol = url.scheme();

//     if protocol == "node" {
//       return Ok(url);
//     }

//     if protocol != "file" && protocol != "data" {
//       return Err(generic_error(format!("Only file and data URLs are supported by the default ESM loader. Received protocol '{}'", protocol)));
//     }

//     // In Deno there's no way to expose internal Node modules anyway,
//     // so calls to NativeModule.canBeRequiredByUsers would only work for built-in modules.

//     if referrer.starts_with("data:") {
//       let referrer_url = Url::parse(referrer)?;
//       return referrer_url.join(specifier).map_err(AnyError::from);
//     }

//     let referrer = if is_main {
//       // path_to_file_url()
//       referrer
//     } else {
//       referrer
//     };

//     let url = module_resolve(specifier, referrer)?;

//     // TODO: check codes

//     Ok(url)
//   }

//   // Ok(module_specifier)
//   todo!()
// }

fn should_be_treated_as_relative_or_absolute_path(specifier: &str) -> bool {
  if specifier == "" {
    return false;
  }

  if specifier.chars().nth(0) == Some('/') {
    return true;
  }

  is_relative_specifier(specifier)
}

fn is_relative_specifier(specifier: &str) -> bool {
  let specifier_len = specifier.len();
  let mut specifier_chars = specifier.chars();

  if specifier_chars.nth(0) == Some('.') {
    if specifier_len == 1 || specifier_chars.nth(1) == Some('/') {
      return true;
    }
    if specifier_chars.nth(1) == Some('.') {
      if specifier_len == 2 || specifier_chars.nth(2) == Some('/') {
        return true;
      }
    }
  }
  false
}

fn module_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let resolved = if should_be_treated_as_relative_or_absolute_path(specifier) {
    base.join(specifier)?
  } else if specifier.chars().nth(0) == Some('#') {
    package_imports_resolve(specifier, base)?
  } else {
    if let Ok(resolved) = Url::parse(specifier) {
      resolved
    } else {
      package_resolve(specifier, base)?
    }
  };
  finalize_resolution(resolved, base)
}

fn finalize_resolution(
  resolved: ModuleSpecifier,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let encoded_sep_re = Regex::new(r"%2F|%2C").expect("bad regex");

  if encoded_sep_re.is_match(resolved.path()) {
    return Err(generic_error(format!(
      "{} must not include encoded \"/\" or \"\\\\\" characters {}",
      resolved.path(),
      base.to_file_path().unwrap().display()
    )));
  }

  let path = resolved.to_file_path().unwrap();

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

  let stats = std::fs::metadata(&p)?;
  if stats.is_dir() {
    return Err(
      generic_error(
        format!("Directory import {} is not supported resolving ES modules imported from {}",
          path.display(), base.to_file_path().unwrap().display()
        )
    ));
  } else if !stats.is_file() {
    return Err(generic_error(format!(
      "Cannot find module {} imported from {}",
      path.display(),
      base.to_file_path().unwrap().display()
    )));
  }

  Ok(resolved)
}

fn package_imports_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  todo!()
}

fn package_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let (package_name, package_subpath, is_scoped) =
    parse_package_name(specifier, base)?;

  // ResolveSelf
  // let package_config = get_package_scope_config(base);

  todo!()
}

fn parse_package_name(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<(String, String, bool), AnyError> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = false;
  let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else {
    if specifier.chars().nth(0) == Some('@') {
      is_scoped = true;
      if let Some(index) = separator_index {
        separator_index = specifier[index + 1..].find('/');
      } else {
        valid_package_name = false;
      }
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
    return Err(generic_error(format!(
      "{} is not a valid package name {}",
      specifier,
      base.to_file_path().unwrap().display()
    )));
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
      .to_string()
  } else {
    ".".to_string()
  };

  Ok((package_name, package_subpath, is_scoped))
}

// enum ExportConfig {
//   Str(String),
//   StrArray(Vec<String>),
// }

// enum PackageType {
//   Module,
//   CommonJs,
// }

// struct PackageConfig {
//   exports: Option<ExportConfig>,
//   name: Option<String>,
//   main: Option<String>,
//   typ: Option<PackageType>
// }

// fn get_package_scope_config(resolved: &str) {
//   todo!()
// }
