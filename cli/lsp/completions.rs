// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::language_server;

use crate::media_type::MediaType;

use deno_core::url::Position;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use std::collections::HashSet;
use std::rc::Rc;
use swc_common::Loc;
use swc_common::SourceMap;
use swc_common::DUMMY_SP;
use swc_ecmascript::ast as swc_ast;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;
use swc_ecmascript::visit::VisitWith;

const CURRENT_PATH: &str = ".";
const PARENT_PATH: &str = "..";

/// Given a specifier, a position, and a snapshot, optionally return a
/// completion response, which will be valid import completions for the specific
/// context.
pub fn get_import_completions(
  specifier: &ModuleSpecifier,
  position: &lsp::Position,
  state_snapshot: &language_server::StateSnapshot,
) -> Option<lsp::CompletionResponse> {
  if let Ok(Some(source)) = state_snapshot.documents.content(specifier) {
    let media_type = MediaType::from(specifier);
    if let Some(current_specifier) =
      is_module_specifier_position(specifier, &source, &media_type, position)
    {
      let workspace_specifiers = get_workspace_specifiers(state_snapshot);
      let specifier_strings =
        get_relative_specifiers(specifier, workspace_specifiers);
      let items = specifier_strings
        .into_iter()
        .filter_map(|label| {
          if label.starts_with(&current_specifier) {
            Some(lsp::CompletionItem {
              kind: Some(lsp::CompletionItemKind::File),
              detail: Some(
                if label.starts_with("http:") || label.starts_with("https:") {
                  "(remote)".to_string()
                } else if label.starts_with("data:") {
                  "(data)".to_string()
                } else {
                  "(local)".to_string()
                },
              ),
              sort_text: Some("1".to_string()),
              insert_text: Some(label.replace(&current_specifier, "")),
              label,
              insert_text_mode: Some(lsp::InsertTextMode::AsIs),
              ..Default::default()
            })
          } else {
            None
          }
        })
        .collect();
      return Some(lsp::CompletionResponse::List(lsp::CompletionList {
        is_incomplete: false,
        items,
      }));
    }
  }
  None
}

fn get_relative_specifiers(
  base: &ModuleSpecifier,
  specifiers: Vec<ModuleSpecifier>,
) -> Vec<String> {
  specifiers
    .iter()
    .filter_map(|s| {
      if s != base {
        Some(relative_specifier(s, base))
      } else {
        None
      }
    })
    .collect()
}

fn get_workspace_specifiers(
  state_snapshot: &language_server::StateSnapshot,
) -> Vec<ModuleSpecifier> {
  let mut specifiers: HashSet<ModuleSpecifier> = state_snapshot
    .documents
    .iter()
    .map(|(specifier, _)| specifier.clone())
    .collect();
  specifiers.extend(state_snapshot.sources.specifiers().into_iter());
  specifiers.into_iter().collect()
}

/// A structure that implements the visit trait to determine if the supplied
/// position falls within the module specifier of an import/export statement.
/// Once the module has been visited,
struct ImportLocator {
  pub maybe_specifier: Option<String>,
  position: lsp::Position,
  source_map: Rc<SourceMap>,
}

impl ImportLocator {
  pub fn new(position: lsp::Position, source_map: Rc<SourceMap>) -> Self {
    Self {
      maybe_specifier: None,
      position,
      source_map,
    }
  }
}

impl Visit for ImportLocator {
  fn visit_import_decl(
    &mut self,
    node: &swc_ast::ImportDecl,
    _parent: &dyn Node,
  ) {
    if self.maybe_specifier.is_none() {
      let start = self.source_map.lookup_char_pos(node.src.span.lo);
      let end = self.source_map.lookup_char_pos(node.src.span.hi);
      if span_includes_pos(&self.position, &start, &end) {
        self.maybe_specifier = Some(node.src.value.to_string());
      }
    }
  }

  fn visit_named_export(
    &mut self,
    node: &swc_ast::NamedExport,
    _parent: &dyn Node,
  ) {
    if self.maybe_specifier.is_none() {
      if let Some(src) = &node.src {
        let start = self.source_map.lookup_char_pos(src.span.lo);
        let end = self.source_map.lookup_char_pos(src.span.hi);
        if span_includes_pos(&self.position, &start, &end) {
          self.maybe_specifier = Some(src.value.to_string());
        }
      }
    }
  }

  fn visit_export_all(
    &mut self,
    node: &swc_ast::ExportAll,
    _parent: &dyn Node,
  ) {
    if self.maybe_specifier.is_none() {
      let start = self.source_map.lookup_char_pos(node.src.span.lo);
      let end = self.source_map.lookup_char_pos(node.src.span.hi);
      if span_includes_pos(&self.position, &start, &end) {
        self.maybe_specifier = Some(node.src.value.to_string());
      }
    }
  }

  fn visit_ts_import_type(
    &mut self,
    node: &swc_ast::TsImportType,
    _parent: &dyn Node,
  ) {
    if self.maybe_specifier.is_none() {
      let start = self.source_map.lookup_char_pos(node.arg.span.lo);
      let end = self.source_map.lookup_char_pos(node.arg.span.hi);
      if span_includes_pos(&self.position, &start, &end) {
        self.maybe_specifier = Some(node.arg.value.to_string());
      }
    }
  }
}

/// Determine if the provided position falls into an module specifier of an
/// import/export statement, optionally returning the current value of the
/// specifier.
fn is_module_specifier_position(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: &MediaType,
  position: &lsp::Position,
) -> Option<String> {
  if let Ok(parsed_module) =
    analysis::parse_module(specifier, source, media_type)
  {
    let mut import_locator =
      ImportLocator::new(*position, parsed_module.source_map.clone());
    parsed_module
      .module
      .visit_with(&swc_ast::Invalid { span: DUMMY_SP }, &mut import_locator);
    import_locator.maybe_specifier
  } else {
    None
  }
}

fn relative_specifier(
  specifier: &ModuleSpecifier,
  base: &ModuleSpecifier,
) -> String {
  if specifier.cannot_be_a_base()
    || base.cannot_be_a_base()
    || specifier.scheme() != base.scheme()
    || specifier.host() != base.host()
    || specifier.port_or_known_default() != base.port_or_known_default()
  {
    if specifier.scheme() == "file" {
      specifier.to_file_path().unwrap().to_string_lossy().into()
    } else {
      specifier.as_str().into()
    }
  } else if let (Some(iter_a), Some(iter_b)) =
    (specifier.path_segments(), base.path_segments())
  {
    let mut vec_a: Vec<&str> = iter_a.collect();
    let mut vec_b: Vec<&str> = iter_b.collect();
    let last_a = if !specifier.path().ends_with('/') && !vec_a.is_empty() {
      vec_a.pop().unwrap()
    } else {
      ""
    };
    let is_dir_b = base.path().ends_with('/');
    if !is_dir_b && !vec_b.is_empty() {
      vec_b.pop();
    }
    if !vec_a.is_empty() && !vec_b.is_empty() && base.path() != "/" {
      let mut parts: Vec<&str> = Vec::new();
      let mut segments_a = vec_a.into_iter();
      let mut segments_b = vec_b.into_iter();
      loop {
        match (segments_a.next(), segments_b.next()) {
          (None, None) => break,
          (Some(a), None) => {
            if parts.is_empty() {
              parts.push(CURRENT_PATH);
            }
            parts.push(a);
            parts.extend(segments_a.by_ref());
            break;
          }
          (None, _) if is_dir_b => parts.push(CURRENT_PATH),
          (None, _) => parts.push(PARENT_PATH),
          (Some(a), Some(b)) if parts.is_empty() && a == b => (),
          (Some(a), Some(b)) if b == CURRENT_PATH => parts.push(a),
          (Some(_), Some(b)) if b == PARENT_PATH => {
            return specifier[Position::BeforePath..].to_string()
          }
          (Some(a), Some(_)) => {
            if parts.is_empty() && is_dir_b {
              parts.push(CURRENT_PATH);
            } else {
              parts.push(PARENT_PATH);
            }
            // actually the clippy suggestions here are less readable for once
            for _ in segments_b {
              #[allow(clippy::same_item_push)]
              parts.push(PARENT_PATH);
            }
            parts.push(a);
            parts.extend(segments_a.by_ref());
            break;
          }
        }
      }
      if parts.is_empty() {
        format!(
          "./{}{}",
          last_a,
          specifier[Position::AfterPath..].to_string()
        )
      } else {
        parts.push(last_a);
        format!(
          "{}{}",
          parts.join("/"),
          specifier[Position::AfterPath..].to_string()
        )
      }
    } else {
      specifier[Position::BeforePath..].into()
    }
  } else {
    specifier[Position::BeforePath..].into()
  }
}

/// Does the position fall within the start and end location?
fn span_includes_pos(position: &lsp::Position, start: &Loc, end: &Loc) -> bool {
  (position.line > (start.line - 1) as u32
    || position.line == (start.line - 1) as u32
      && position.character >= start.col_display as u32)
    && (position.line < (end.line - 1) as u32
      || position.line == (end.line - 1) as u32
        && position.character <= end.col_display as u32)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::lsp::analysis;
  use crate::lsp::documents::DocumentCache;
  use crate::lsp::sources::Sources;
  use crate::media_type::MediaType;
  use deno_core::resolve_url;
  use std::path::Path;
  use tempfile::TempDir;

  fn mock_state_snapshot(
    fixtures: &[(&str, &str, i32)],
    location: &Path,
  ) -> language_server::StateSnapshot {
    let mut documents = DocumentCache::default();
    for (specifier, source, version) in fixtures {
      let specifier =
        resolve_url(specifier).expect("failed to create specifier");
      documents.open(specifier.clone(), *version, source);
      let media_type = MediaType::from(&specifier);
      let parsed_module =
        analysis::parse_module(&specifier, source, &media_type).unwrap();
      let (deps, _) = analysis::analyze_dependencies(
        &specifier,
        &media_type,
        &parsed_module,
        &None,
      );
      documents.set_dependencies(&specifier, Some(deps)).unwrap();
    }
    let sources = Sources::new(location);
    language_server::StateSnapshot {
      documents,
      sources,
      ..Default::default()
    }
  }

  fn setup(documents: &[(&str, &str, i32)]) -> language_server::StateSnapshot {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let location = temp_dir.path().join("deps");
    mock_state_snapshot(documents, &location)
  }

  #[test]
  fn test_get_workspace_specifiers() {
    let state_snapshot =
      setup(&[("file:///a.ts", r#"console.log("hello deno");"#, 1)]);
    let result = get_workspace_specifiers(&state_snapshot);
    assert_eq!(result, vec![resolve_url("file:///a.ts").unwrap()]);
  }

  #[test]
  fn test_get_relative_specifiers() {
    let base = resolve_url("file:///a/b/c.ts").unwrap();
    let specifiers = vec![
      resolve_url("file:///a/b/c.ts").unwrap(),
      resolve_url("file:///a/b/d.ts").unwrap(),
      resolve_url("file:///a/c/c.ts").unwrap(),
      resolve_url("file:///a/b/d/d.ts").unwrap(),
      resolve_url("https://deno.land/x/a/b/c.ts").unwrap(),
    ];
    assert_eq!(
      get_relative_specifiers(&base, specifiers),
      vec![
        "./d.ts".to_string(),
        "../c/c.ts".to_string(),
        "./d/d.ts".to_string(),
        "https://deno.land/x/a/b/c.ts".to_string(),
      ]
    );
  }

  #[test]
  fn test_relative_specifier() {
    let fixtures: Vec<(&str, &str, &str)> = vec![
      (
        "https://deno.land/x/a/b/c.ts",
        "https://deno.land/x/a/b/d.ts",
        "./c.ts",
      ),
      (
        "https://deno.land/x/a/c.ts",
        "https://deno.land/x/a/b/d.ts",
        "../c.ts",
      ),
      (
        "https://deno.land/x/a/b/c/d.ts",
        "https://deno.land/x/a/b/d.ts",
        "./c/d.ts",
      ),
      (
        "https://deno.land/x/a/b/c/d.ts",
        "https://deno.land/x/a/b/c/",
        "./d.ts",
      ),
      (
        "https://deno.land/x/a/b/c/d/e.ts",
        "https://deno.land/x/a/b/c/",
        "./d/e.ts",
      ),
      (
        "https://deno.land/x/a/b/c/d/e.ts",
        "https://deno.land/x/a/b/c/f.ts",
        "./d/e.ts",
      ),
      (
        "https://deno.land/x/a/c.ts?foo=bar",
        "https://deno.land/x/a/b/d.ts",
        "../c.ts?foo=bar",
      ),
      (
        "https://deno.land/x/a/b/c.ts",
        "https://deno.land/x/a/b/d.ts?foo=bar",
        "./c.ts",
      ),
      ("file:///a/b/c.ts", "file:///a/b/d.ts", "./c.ts"),
      (
        "file:///a/b/c.ts",
        "https://deno.land/x/a/b/c.ts",
        "/a/b/c.ts",
      ),
      (
        "https://deno.land/x/a/b/c.ts",
        "https://deno.land/",
        "/x/a/b/c.ts",
      ),
      (
        "https://deno.land/x/a/b/c.ts",
        "https://deno.land/x/d/e/f.ts",
        "../../a/b/c.ts",
      ),
    ];
    for (specifier_str, base_str, expected) in fixtures {
      let specifier = resolve_url(specifier_str).unwrap();
      let base = resolve_url(base_str).unwrap();
      let actual = relative_specifier(&specifier, &base);
      assert_eq!(
        actual, expected,
        "specifier: \"{}\" base: \"{}\"",
        specifier_str, base_str
      );
    }
  }

  #[test]
  fn test_is_module_specifier_position() {
    let specifier = resolve_url("file:///a/b/c.ts").unwrap();
    let source = r#"import * as a from """#;
    let media_type = MediaType::TypeScript;
    assert_eq!(
      is_module_specifier_position(
        &specifier,
        source,
        &media_type,
        &lsp::Position {
          line: 0,
          character: 0
        }
      ),
      None
    );
    assert_eq!(
      is_module_specifier_position(
        &specifier,
        source,
        &media_type,
        &lsp::Position {
          line: 0,
          character: 20
        }
      ),
      Some("".to_string())
    );
  }

  #[test]
  fn test_is_module_specifier_position_partial() {
    let specifier = resolve_url("file:///a/b/c.ts").unwrap();
    let source = r#"import * as a from "https://""#;
    let media_type = MediaType::TypeScript;
    assert_eq!(
      is_module_specifier_position(
        &specifier,
        source,
        &media_type,
        &lsp::Position {
          line: 0,
          character: 0
        }
      ),
      None
    );
    assert_eq!(
      is_module_specifier_position(
        &specifier,
        source,
        &media_type,
        &lsp::Position {
          line: 0,
          character: 28
        }
      ),
      Some("https://".to_string())
    );
  }

  #[test]
  fn test_get_import_completions() {
    let specifier = resolve_url("file:///a/b/c.ts").unwrap();
    let position = lsp::Position {
      line: 0,
      character: 20,
    };
    let state_snapshot = setup(&[
      ("file:///a/b/c.ts", r#"import * as d from """#, 1),
      ("file:///a/c.ts", r#""#, 1),
    ]);
    let actual = get_import_completions(&specifier, &position, &state_snapshot);
    assert_eq!(
      actual,
      Some(lsp::CompletionResponse::List(lsp::CompletionList {
        is_incomplete: false,
        items: vec![lsp::CompletionItem {
          label: "../c.ts".to_string(),
          kind: Some(lsp::CompletionItemKind::File),
          detail: Some("(local)".to_string()),
          sort_text: Some("1".to_string()),
          insert_text: Some("../c.ts".to_string()),
          insert_text_mode: Some(lsp::InsertTextMode::AsIs),
          ..Default::default()
        }]
      }))
    );
  }
}
