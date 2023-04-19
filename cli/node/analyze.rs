// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::swc::common::SyntaxContext;
use deno_ast::view::Node;
use deno_ast::view::NodeTrait;
use deno_ast::CjsAnalysis;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceRanged;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_runtime::deno_node::package_exports_resolve;
use deno_runtime::deno_node::NodeModuleKind;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_node::PathClean;
use deno_runtime::deno_node::RealFs;
use deno_runtime::deno_node::RequireNpmResolver;
use deno_runtime::deno_node::NODE_GLOBAL_THIS_NAME;
use once_cell::sync::Lazy;

use crate::cache::NodeAnalysisCache;
use crate::file_fetcher::FileFetcher;
use crate::npm::NpmPackageResolver;

static NODE_GLOBALS: &[&str] = &[
  "Buffer",
  "clearImmediate",
  "clearInterval",
  "clearTimeout",
  "console",
  "global",
  "process",
  "setImmediate",
  "setInterval",
  "setTimeout",
];

pub struct NodeCodeTranslator {
  analysis_cache: NodeAnalysisCache,
  file_fetcher: Arc<FileFetcher>,
  npm_resolver: Arc<NpmPackageResolver>,
}

impl NodeCodeTranslator {
  pub fn new(
    analysis_cache: NodeAnalysisCache,
    file_fetcher: Arc<FileFetcher>,
    npm_resolver: Arc<NpmPackageResolver>,
  ) -> Self {
    Self {
      analysis_cache,
      file_fetcher,
      npm_resolver,
    }
  }

  pub fn esm_code_with_node_globals(
    &self,
    specifier: &ModuleSpecifier,
    code: String,
  ) -> Result<String, AnyError> {
    esm_code_with_node_globals(&self.analysis_cache, specifier, code)
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
    code: String,
    media_type: MediaType,
    permissions: &mut dyn NodePermissions,
  ) -> Result<String, AnyError> {
    let mut temp_var_count = 0;
    let mut handled_reexports: HashSet<String> = HashSet::default();

    let mut source = vec![
      r#"import {createRequire as __internalCreateRequire} from "node:module";
      const require = __internalCreateRequire(import.meta.url);"#
        .to_string(),
    ];

    let analysis =
      self.perform_cjs_analysis(specifier.as_str(), media_type, code)?;

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
      let reexport_specifier =
        ModuleSpecifier::from_file_path(resolved_reexport).unwrap();
      // Second, read the source code from disk
      let reexport_file = self
        .file_fetcher
        .get_source(&reexport_specifier)
        .ok_or_else(|| {
          anyhow!(
            "Could not find '{}' ({}) referenced from {}",
            reexport,
            reexport_specifier,
            referrer
          )
        })?;

      {
        let analysis = self.perform_cjs_analysis(
          reexport_specifier.as_str(),
          reexport_file.media_type,
          reexport_file.source.to_string(),
        )?;

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

  fn perform_cjs_analysis(
    &self,
    specifier: &str,
    media_type: MediaType,
    code: String,
  ) -> Result<CjsAnalysis, AnyError> {
    let source_hash = NodeAnalysisCache::compute_source_hash(&code);
    if let Some(analysis) = self
      .analysis_cache
      .get_cjs_analysis(specifier, &source_hash)
    {
      return Ok(analysis);
    }

    if media_type == MediaType::Json {
      return Ok(CjsAnalysis {
        exports: vec![],
        reexports: vec![],
      });
    }

    let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      text_info: deno_ast::SourceTextInfo::new(code.into()),
      media_type,
      capture_tokens: true,
      scope_analysis: false,
      maybe_syntax: None,
    })?;
    let analysis = parsed_source.analyze_cjs();
    self
      .analysis_cache
      .set_cjs_analysis(specifier, &source_hash, &analysis);

    Ok(analysis)
  }

  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    conditions: &[&str],
    mode: NodeResolutionMode,
    permissions: &mut dyn NodePermissions,
  ) -> Result<PathBuf, AnyError> {
    if specifier.starts_with('/') {
      todo!();
    }

    let referrer_path = referrer.to_file_path().unwrap();
    if specifier.starts_with("./") || specifier.starts_with("../") {
      if let Some(parent) = referrer_path.parent() {
        return file_extension_probe(parent.join(specifier), &referrer_path);
      } else {
        todo!();
      }
    }

    // We've got a bare specifier or maybe bare_specifier/blah.js"

    let (package_specifier, package_subpath) =
      parse_specifier(specifier).unwrap();

    // todo(dsherret): use not_found error on not found here
    let resolver = self.npm_resolver.as_require_npm_resolver();
    let module_dir = resolver.resolve_package_folder_from_package(
      package_specifier.as_str(),
      &referrer_path,
      mode,
    )?;

    let package_json_path = module_dir.join("package.json");
    if package_json_path.exists() {
      let package_json = PackageJson::load::<RealFs>(
        &self.npm_resolver.as_require_npm_resolver(),
        permissions,
        package_json_path.clone(),
      )?;

      if let Some(exports) = &package_json.exports {
        return package_exports_resolve::<RealFs>(
          &package_json_path,
          package_subpath,
          exports,
          referrer,
          NodeModuleKind::Esm,
          conditions,
          mode,
          &self.npm_resolver.as_require_npm_resolver(),
          permissions,
        );
      }

      // old school
      if package_subpath != "." {
        let d = module_dir.join(package_subpath);
        if let Ok(m) = d.metadata() {
          if m.is_dir() {
            // subdir might have a package.json that specifies the entrypoint
            let package_json_path = d.join("package.json");
            if package_json_path.exists() {
              let package_json = PackageJson::load::<RealFs>(
                &self.npm_resolver.as_require_npm_resolver(),
                permissions,
                package_json_path,
              )?;
              if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
                return Ok(d.join(main).clean());
              }
            }

            return Ok(d.join("index.js").clean());
          }
        }
        return file_extension_probe(d, &referrer_path);
      } else if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
        return Ok(module_dir.join(main).clean());
      } else {
        return Ok(module_dir.join("index.js").clean());
      }
    }
    Err(not_found(specifier, &referrer_path))
  }
}

fn esm_code_with_node_globals(
  analysis_cache: &NodeAnalysisCache,
  specifier: &ModuleSpecifier,
  code: String,
) -> Result<String, AnyError> {
  // TODO(dsherret): this code is way more inefficient than it needs to be.
  //
  // In the future, we should disable capturing tokens & scope analysis
  // and instead only use swc's APIs to go through the portions of the tree
  // that we know will affect the global scope while still ensuring that
  // `var` decls are taken into consideration.
  let source_hash = NodeAnalysisCache::compute_source_hash(&code);
  let text_info = deno_ast::SourceTextInfo::from_string(code);
  let top_level_decls = if let Some(decls) =
    analysis_cache.get_esm_analysis(specifier.as_str(), &source_hash)
  {
    HashSet::from_iter(decls)
  } else {
    let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      text_info: text_info.clone(),
      media_type: deno_ast::MediaType::from_specifier(specifier),
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })?;
    let top_level_decls = analyze_top_level_decls(&parsed_source)?;
    analysis_cache.set_esm_analysis(
      specifier.as_str(),
      &source_hash,
      &top_level_decls.clone().into_iter().collect(),
    );
    top_level_decls
  };

  Ok(esm_code_from_top_level_decls(
    text_info.text_str(),
    &top_level_decls,
  ))
}

fn esm_code_from_top_level_decls(
  file_text: &str,
  top_level_decls: &HashSet<String>,
) -> String {
  let mut globals = Vec::with_capacity(NODE_GLOBALS.len());
  let has_global_this = top_level_decls.contains("globalThis");
  for global in NODE_GLOBALS.iter() {
    if !top_level_decls.contains(&global.to_string()) {
      globals.push(*global);
    }
  }

  let mut result = String::new();
  let global_this_expr = NODE_GLOBAL_THIS_NAME.as_str();
  let global_this_expr = if has_global_this {
    global_this_expr
  } else {
    write!(result, "var globalThis = {global_this_expr};").unwrap();
    "globalThis"
  };
  for global in globals {
    write!(result, "var {global} = {global_this_expr}.{global};").unwrap();
  }

  // strip the shebang
  let file_text = if file_text.starts_with("#!/") {
    let start_index = file_text.find('\n').unwrap_or(file_text.len());
    &file_text[start_index..]
  } else {
    file_text
  };
  result.push_str(file_text);

  result
}

fn analyze_top_level_decls(
  parsed_source: &ParsedSource,
) -> Result<HashSet<String>, AnyError> {
  fn visit_children(
    node: Node,
    top_level_context: SyntaxContext,
    results: &mut HashSet<String>,
  ) {
    if let Node::Ident(ident) = node {
      if ident.ctxt() == top_level_context && is_local_declaration_ident(node) {
        results.insert(ident.sym().to_string());
      }
    }

    for child in node.children() {
      visit_children(child, top_level_context, results);
    }
  }

  let top_level_context = parsed_source.top_level_context();

  parsed_source.with_view(|program| {
    let mut results = HashSet::new();
    visit_children(program.into(), top_level_context, &mut results);
    Ok(results)
  })
}

fn is_local_declaration_ident(node: Node) -> bool {
  if let Some(parent) = node.parent() {
    match parent {
      Node::BindingIdent(decl) => decl.id.range().contains(&node.range()),
      Node::ClassDecl(decl) => decl.ident.range().contains(&node.range()),
      Node::ClassExpr(decl) => decl
        .ident
        .as_ref()
        .map(|i| i.range().contains(&node.range()))
        .unwrap_or(false),
      Node::TsInterfaceDecl(decl) => decl.id.range().contains(&node.range()),
      Node::FnDecl(decl) => decl.ident.range().contains(&node.range()),
      Node::FnExpr(decl) => decl
        .ident
        .as_ref()
        .map(|i| i.range().contains(&node.range()))
        .unwrap_or(false),
      Node::TsModuleDecl(decl) => decl.id.range().contains(&node.range()),
      Node::TsNamespaceDecl(decl) => decl.id.range().contains(&node.range()),
      Node::VarDeclarator(decl) => decl.name.range().contains(&node.range()),
      Node::ImportNamedSpecifier(decl) => {
        decl.local.range().contains(&node.range())
      }
      Node::ImportDefaultSpecifier(decl) => {
        decl.local.range().contains(&node.range())
      }
      Node::ImportStarAsSpecifier(decl) => decl.range().contains(&node.range()),
      Node::KeyValuePatProp(decl) => decl.key.range().contains(&node.range()),
      Node::AssignPatProp(decl) => decl.key.range().contains(&node.range()),
      _ => false,
    }
  } else {
    false
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

fn file_extension_probe(
  p: PathBuf,
  referrer: &Path,
) -> Result<PathBuf, AnyError> {
  let p = p.clean();
  if p.exists() {
    let file_name = p.file_name().unwrap();
    let p_js = p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
    if p_js.exists() && p_js.is_file() {
      return Ok(p_js);
    } else if p.is_dir() {
      return Ok(p.join("index.js"));
    } else {
      return Ok(p);
    }
  } else if let Some(file_name) = p.file_name() {
    let p_js = p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
    if p_js.exists() && p_js.is_file() {
      return Ok(p_js);
    }
  }
  Err(not_found(&p.to_string_lossy(), referrer))
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
  fn test_esm_code_with_node_globals() {
    let r = esm_code_with_node_globals(
      &NodeAnalysisCache::new_in_memory(),
      &ModuleSpecifier::parse("https://example.com/foo/bar.js").unwrap(),
      "export const x = 1;".to_string(),
    )
    .unwrap();
    assert!(r.contains(&format!(
      "var globalThis = {};",
      NODE_GLOBAL_THIS_NAME.as_str()
    )));
    assert!(r.contains("var process = globalThis.process;"));
    assert!(r.contains("export const x = 1;"));
  }

  #[test]
  fn test_esm_code_with_node_globals_with_shebang() {
    let r = esm_code_with_node_globals(
      &NodeAnalysisCache::new_in_memory(),
      &ModuleSpecifier::parse("https://example.com/foo/bar.js").unwrap(),
      "#!/usr/bin/env node\nexport const x = 1;".to_string(),
    )
    .unwrap();
    assert_eq!(
      r,
      format!(
        concat!(
          "var globalThis = {}",
          ";var Buffer = globalThis.Buffer;",
          "var clearImmediate = globalThis.clearImmediate;var clearInterval = globalThis.clearInterval;",
          "var clearTimeout = globalThis.clearTimeout;var console = globalThis.console;",
          "var global = globalThis.global;var process = globalThis.process;",
          "var setImmediate = globalThis.setImmediate;var setInterval = globalThis.setInterval;",
          "var setTimeout = globalThis.setTimeout;\n",
          "export const x = 1;"
        ),
        NODE_GLOBAL_THIS_NAME.as_str(),
      )
    );
  }

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
