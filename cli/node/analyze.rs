// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;

use deno_ast::swc::common::SyntaxContext;
use deno_ast::view::Node;
use deno_ast::view::NodeTrait;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceRanged;
use deno_core::error::AnyError;
use deno_runtime::deno_node::NODE_GLOBAL_THIS_NAME;
use std::fmt::Write;

static NODE_GLOBALS: &[&str] = &[
  "Buffer",
  "clearImmediate",
  "clearInterval",
  "clearTimeout",
  "global",
  "process",
  "setImmediate",
  "setInterval",
  "setTimeout",
];

// TODO(dsherret): this code is way more inefficient than it needs to be.
//
// In the future, we should disable capturing tokens & scope analysis
// and instead only use swc's APIs to go through the portions of the tree
// that we know will affect the global scope while still ensuring that
// `var` decls are taken into consideration.

pub fn esm_code_with_node_globals(
  specifier: &ModuleSpecifier,
  code: String,
) -> Result<String, AnyError> {
  let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
    specifier: specifier.to_string(),
    text_info: deno_ast::SourceTextInfo::from_string(code),
    media_type: deno_ast::MediaType::from(specifier),
    capture_tokens: true,
    scope_analysis: true,
    maybe_syntax: None,
  })?;
  let top_level_decls = analyze_top_level_decls(&parsed_source)?;
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
    write!(result, "var globalThis = {};", global_this_expr).unwrap();
    "globalThis"
  };
  for global in globals {
    write!(result, "var {0} = {1}.{0};", global, global_this_expr).unwrap();
  }

  let file_text = parsed_source.text_info().text_str();
  // strip the shebang
  let file_text = if file_text.starts_with("#!/") {
    let start_index = file_text.find('\n').unwrap_or(file_text.len());
    &file_text[start_index..]
  } else {
    file_text
  };
  result.push_str(file_text);

  Ok(result)
}

fn analyze_top_level_decls(
  parsed_source: &ParsedSource,
) -> Result<HashSet<String>, AnyError> {
  let top_level_context = parsed_source.top_level_context();

  parsed_source.with_view(|program| {
    let mut results = HashSet::new();
    visit_children(program.into(), top_level_context, &mut results);
    Ok(results)
  })
}

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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_esm_code_with_node_globals() {
    let r = esm_code_with_node_globals(
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
          "var clearTimeout = globalThis.clearTimeout;var global = globalThis.global;",
          "var process = globalThis.process;var setImmediate = globalThis.setImmediate;",
          "var setInterval = globalThis.setInterval;var setTimeout = globalThis.setTimeout;\n",
          "export const x = 1;"
        ),
        NODE_GLOBAL_THIS_NAME.as_str(),
      )
    );
  }
}
