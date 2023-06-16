// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::ModuleSpecifier;
use once_cell::sync::Lazy;

use deno_core::error::AnyError;

use crate::resolution::NodeResolverRc;
use crate::NodeModuleKind;
use crate::NodePermissions;
use crate::NodeResolutionMode;
use crate::NpmResolverRc;
use crate::PackageJson;
use crate::PathClean;

#[derive(Debug, Clone)]
pub struct CjsAnalysis {
  pub exports: Vec<String>,
  pub reexports: Vec<String>,
}

/// Code analyzer for CJS and ESM files.
pub trait CjsCodeAnalyzer {
  /// Analyzes CommonJs code for exports and reexports, which is
  /// then used to determine the wrapper ESM module exports.
  fn analyze_cjs(
    &self,
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> Result<CjsAnalysis, AnyError>;
}

pub struct NodeCodeTranslator<TCjsCodeAnalyzer: CjsCodeAnalyzer> {
  cjs_esm_code_analyzer: TCjsCodeAnalyzer,
  fs: deno_fs::FileSystemRc,
  node_resolver: NodeResolverRc,
  npm_resolver: NpmResolverRc,
}

impl<TCjsCodeAnalyzer: CjsCodeAnalyzer> NodeCodeTranslator<TCjsCodeAnalyzer> {
  pub fn new(
    cjs_esm_code_analyzer: TCjsCodeAnalyzer,
    fs: deno_fs::FileSystemRc,
    node_resolver: NodeResolverRc,
    npm_resolver: NpmResolverRc,
  ) -> Self {
    Self {
      cjs_esm_code_analyzer,
      fs,
      node_resolver,
      npm_resolver,
    }
  }

  /// Translates given CJS module into ESM. This function will perform static
  /// analysis on the file to find defined exports and reexports.
  ///
  /// For all discovered reexports the analysis will be performed recursively.
  ///
  /// If successful a source code for equivalent ES module is returned.
  pub fn translate_cjs_to_esm(
    &self,
    specifier: &ModuleSpecifier,
    source: &str,
    permissions: &dyn NodePermissions,
  ) -> Result<String, AnyError> {
    let mut temp_var_count = 0;
    let mut handled_reexports: HashSet<String> = HashSet::default();

    let analysis = self.cjs_esm_code_analyzer.analyze_cjs(specifier, source)?;

    let mut source = vec![
      r#"import {createRequire as __internalCreateRequire} from "node:module";
      const require = __internalCreateRequire(import.meta.url);"#
        .to_string(),
    ];

    let mut all_exports = analysis
      .exports
      .iter()
      .map(|s| s.to_string())
      .collect::<HashSet<_>>();

    // (request, referrer)
    let mut reexports_to_handle = VecDeque::new();
    for reexport in analysis.reexports {
      reexports_to_handle.push_back((reexport, specifier.clone()));
    }

    while let Some((reexport, referrer)) = reexports_to_handle.pop_front() {
      if handled_reexports.contains(&reexport) {
        continue;
      }

      handled_reexports.insert(reexport.to_string());

      // First, resolve relate reexport specifier
      let resolved_reexport = self.resolve(
        &reexport,
        &referrer,
        // FIXME(bartlomieju): check if these conditions are okay, probably
        // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
        &["deno", "require", "default"],
        NodeResolutionMode::Execution,
        permissions,
      )?;
      // Second, read the source code from disk
      let reexport_specifier =
        ModuleSpecifier::from_file_path(&resolved_reexport).unwrap();
      let reexport_file_text = self
        .fs
        .read_to_string(&resolved_reexport)
        .map_err(AnyError::from)
        .with_context(|| {
          format!(
            "Could not find '{}' ({}) referenced from {}",
            reexport, reexport_specifier, referrer
          )
        })?;
      {
        let analysis = self
          .cjs_esm_code_analyzer
          .analyze_cjs(&reexport_specifier, &reexport_file_text)?;

        for reexport in analysis.reexports {
          reexports_to_handle.push_back((reexport, reexport_specifier.clone()));
        }

        all_exports.extend(
          analysis
            .exports
            .into_iter()
            .filter(|e| e.as_str() != "default"),
        );
      }
    }

    source.push(format!(
      "const mod = require(\"{}\");",
      specifier
        .to_file_path()
        .unwrap()
        .to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('\'', "\\\'")
        .replace('\"', "\\\"")
    ));

    for export in &all_exports {
      if export.as_str() != "default" {
        add_export(
          &mut source,
          export,
          &format!("mod[\"{export}\"]"),
          &mut temp_var_count,
        );
      }
    }

    source.push("export default mod;".to_string());

    let translated_source = source.join("\n");
    Ok(translated_source)
  }

  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &dyn NodePermissions,
  ) -> Result<PathBuf, AnyError> {
    if specifier.starts_with('/') {
      todo!();
    }

    let referrer_path = referrer.to_file_path().unwrap();
    if specifier.starts_with("./") || specifier.starts_with("../") {
      if let Some(parent) = referrer_path.parent() {
        return self
          .file_extension_probe(parent.join(specifier), &referrer_path);
      } else {
        todo!();
      }
    }

    // We've got a bare specifier or maybe bare_specifier/blah.js"

    let (package_specifier, package_subpath) =
      parse_specifier(specifier).unwrap();

    // todo(dsherret): use not_found error on not found here
    let module_dir = self.npm_resolver.resolve_package_folder_from_package(
      package_specifier.as_str(),
      referrer,
      mode,
    )?;

    let package_json_path = module_dir.join("package.json");
    if self.fs.exists(&package_json_path) {
      let package_json = PackageJson::load(
        &*self.fs,
        &*self.npm_resolver,
        permissions,
        package_json_path.clone(),
      )?;

      if let Some(exports) = &package_json.exports {
        return self.node_resolver.package_exports_resolve(
          &package_json_path,
          package_subpath,
          exports,
          referrer,
          NodeModuleKind::Esm,
          conditions,
          mode,
          permissions,
        );
      }

      // old school
      if package_subpath != "." {
        let d = module_dir.join(package_subpath);
        if self.fs.is_dir(&d) {
          // subdir might have a package.json that specifies the entrypoint
          let package_json_path = d.join("package.json");
          if self.fs.exists(&package_json_path) {
            let package_json = PackageJson::load(
              &*self.fs,
              &*self.npm_resolver,
              permissions,
              package_json_path,
            )?;
            if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
              return Ok(d.join(main).clean());
            }
          }

          return Ok(d.join("index.js").clean());
        }
        return self.file_extension_probe(d, &referrer_path);
      } else if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
        return Ok(module_dir.join(main).clean());
      } else {
        return Ok(module_dir.join("index.js").clean());
      }
    }
    Err(not_found(specifier, &referrer_path))
  }

  fn file_extension_probe(
    &self,
    p: PathBuf,
    referrer: &Path,
  ) -> Result<PathBuf, AnyError> {
    let p = p.clean();
    if self.fs.exists(&p) {
      let file_name = p.file_name().unwrap();
      let p_js =
        p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
      if self.fs.is_file(&p_js) {
        return Ok(p_js);
      } else if self.fs.is_dir(&p) {
        return Ok(p.join("index.js"));
      } else {
        return Ok(p);
      }
    } else if let Some(file_name) = p.file_name() {
      let p_js =
        p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
      if self.fs.is_file(&p_js) {
        return Ok(p_js);
      }
    }
    Err(not_found(&p.to_string_lossy(), referrer))
  }
}

static RESERVED_WORDS: Lazy<HashSet<&str>> = Lazy::new(|| {
  HashSet::from([
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "let",
    "enum",
    "implements",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "static",
  ])
});

fn add_export(
  source: &mut Vec<String>,
  name: &str,
  initializer: &str,
  temp_var_count: &mut usize,
) {
  fn is_valid_var_decl(name: &str) -> bool {
    // it's ok to be super strict here
    name
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
  }

  // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
  // but it might not be necessary here since our analysis is more detailed?
  if RESERVED_WORDS.contains(name) || !is_valid_var_decl(name) {
    *temp_var_count += 1;
    // we can't create an identifier with a reserved word or invalid identifier name,
    // so assign it to a temporary variable that won't have a conflict, then re-export
    // it as a string
    source.push(format!(
      "const __deno_export_{temp_var_count}__ = {initializer};"
    ));
    source.push(format!(
      "export {{ __deno_export_{temp_var_count}__ as \"{name}\" }};"
    ));
  } else {
    source.push(format!("export const {name} = {initializer};"));
  }
}

fn parse_specifier(specifier: &str) -> Option<(String, String)> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = true;
  // let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else if specifier.starts_with('@') {
    // is_scoped = true;
    if let Some(index) = separator_index {
      separator_index = specifier[index + 1..].find('/').map(|i| i + index + 1);
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
    return None;
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
  } else {
    ".".to_string()
  };

  Some((package_name, package_subpath))
}

fn not_found(path: &str, referrer: &Path) -> AnyError {
  let msg = format!(
    "[ERR_MODULE_NOT_FOUND] Cannot find module \"{}\" imported from \"{}\"",
    path,
    referrer.to_string_lossy()
  );
  std::io::Error::new(std::io::ErrorKind::NotFound, msg).into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_add_export() {
    let mut temp_var_count = 0;
    let mut source = vec![];

    let exports = vec!["static", "server", "app", "dashed-export"];
    for export in exports {
      add_export(&mut source, export, "init", &mut temp_var_count);
    }
    assert_eq!(
      source,
      vec![
        "const __deno_export_1__ = init;".to_string(),
        "export { __deno_export_1__ as \"static\" };".to_string(),
        "export const server = init;".to_string(),
        "export const app = init;".to_string(),
        "const __deno_export_2__ = init;".to_string(),
        "export { __deno_export_2__ as \"dashed-export\" };".to_string(),
      ]
    )
  }

  #[test]
  fn test_parse_specifier() {
    assert_eq!(
      parse_specifier("@some-package/core/actions"),
      Some(("@some-package/core".to_string(), "./actions".to_string()))
    );
  }
}
