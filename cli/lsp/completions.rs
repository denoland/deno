// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::language_server;
use super::tsc;

use crate::fs_util::is_supported_ext;
use crate::media_type::MediaType;

use deno_core::normalize_path;
use deno_core::resolve_path;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::url::Position;
use deno_core::ModuleSpecifier;
use lspower::lsp;
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
const LOCAL_PATHS: &[&str] = &[CURRENT_PATH, PARENT_PATH];

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemData {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tsc: Option<tsc::CompletionItemData>,
}

/// Given a specifier, a position, and a snapshot, optionally return a
/// completion response, which will be valid import completions for the specific
/// context.
pub async fn get_import_completions(
  specifier: &ModuleSpecifier,
  position: &lsp::Position,
  state_snapshot: &language_server::StateSnapshot,
) -> Option<lsp::CompletionResponse> {
  if let Ok(Some(source)) = state_snapshot.documents.content(specifier) {
    let media_type = MediaType::from(specifier);
    if let Some((current_specifier, range)) =
      is_module_specifier_position(specifier, &source, &media_type, position)
    {
      // completions for local relative modules
      if current_specifier.starts_with("./")
        || current_specifier.starts_with("../")
      {
        return Some(lsp::CompletionResponse::List(lsp::CompletionList {
          is_incomplete: false,
          items: get_local_completions(specifier, &current_specifier, &range)?,
        }));
      }
      // completion of modules from a module registry or cache
      if !current_specifier.is_empty() {
        let offset = if position.character > range.start.character {
          (position.character - range.start.character) as usize
        } else {
          0
        };
        let maybe_items = state_snapshot
          .module_registries
          .get_completions(&current_specifier, offset, &range, state_snapshot)
          .await;
        let items = maybe_items.unwrap_or_else(|| {
          get_workspace_completions(
            specifier,
            &current_specifier,
            &range,
            state_snapshot,
          )
        });
        return Some(lsp::CompletionResponse::List(lsp::CompletionList {
          is_incomplete: false,
          items,
        }));
      } else {
        let mut items: Vec<lsp::CompletionItem> = LOCAL_PATHS
          .iter()
          .map(|s| lsp::CompletionItem {
            label: s.to_string(),
            kind: Some(lsp::CompletionItemKind::Folder),
            detail: Some("(local)".to_string()),
            sort_text: Some("1".to_string()),
            insert_text: Some(s.to_string()),
            ..Default::default()
          })
          .collect();
        if let Some(origin_items) = state_snapshot
          .module_registries
          .get_origin_completions(&current_specifier, &range)
        {
          items.extend(origin_items);
        }
        return Some(lsp::CompletionResponse::List(lsp::CompletionList {
          is_incomplete: false,
          items,
        }));
        // TODO(@kitsonk) add bare specifiers from import map
      }
    }
  }
  None
}

/// Return local completions that are relative to the base specifier.
fn get_local_completions(
  base: &ModuleSpecifier,
  current: &str,
  range: &lsp::Range,
) -> Option<Vec<lsp::CompletionItem>> {
  if base.scheme() != "file" {
    return None;
  }

  let mut base_path = base.to_file_path().ok()?;
  base_path.pop();
  let mut current_path = normalize_path(base_path.join(current));
  // if the current text does not end in a `/` then we are still selecting on
  // the parent and should show all completions from there.
  let is_parent = if !current.ends_with('/') {
    current_path.pop();
    true
  } else {
    false
  };
  if current_path.is_dir() {
    let items = std::fs::read_dir(current_path).ok()?;
    Some(
      items
        .filter_map(|de| {
          let de = de.ok()?;
          let label = de.path().file_name()?.to_string_lossy().to_string();
          let entry_specifier = resolve_path(de.path().to_str()?).ok()?;
          if &entry_specifier == base {
            return None;
          }
          let full_text = relative_specifier(&entry_specifier, base);
          // this weeds out situations where we are browsing in the parent, but
          // we want to filter out non-matches when the completion is manually
          // invoked by the user, but still allows for things like `../src/../`
          // which is silly, but no reason to not allow it.
          if is_parent && !full_text.starts_with(current) {
            return None;
          }
          let text_edit = Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
            range: *range,
            new_text: full_text.clone(),
          }));
          let filter_text = if full_text.starts_with(current) {
            Some(full_text)
          } else {
            Some(format!("{}{}", current, label))
          };
          match de.file_type() {
            Ok(file_type) if file_type.is_dir() => Some(lsp::CompletionItem {
              label,
              kind: Some(lsp::CompletionItemKind::Folder),
              filter_text,
              sort_text: Some("1".to_string()),
              text_edit,
              ..Default::default()
            }),
            Ok(file_type) if file_type.is_file() => {
              if is_supported_ext(&de.path()) {
                Some(lsp::CompletionItem {
                  label,
                  kind: Some(lsp::CompletionItemKind::File),
                  detail: Some("(local)".to_string()),
                  filter_text,
                  sort_text: Some("1".to_string()),
                  text_edit,
                  ..Default::default()
                })
              } else {
                None
              }
            }
            _ => None,
          }
        })
        .collect(),
    )
  } else {
    None
  }
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

/// Get workspace completions that include modules in the Deno cache which match
/// the current specifier string.
fn get_workspace_completions(
  specifier: &ModuleSpecifier,
  current: &str,
  range: &lsp::Range,
  state_snapshot: &language_server::StateSnapshot,
) -> Vec<lsp::CompletionItem> {
  let workspace_specifiers = state_snapshot.sources.specifiers();
  let specifier_strings =
    get_relative_specifiers(specifier, workspace_specifiers);
  specifier_strings
    .into_iter()
    .filter_map(|label| {
      if label.starts_with(&current) {
        let detail = Some(
          if label.starts_with("http:") || label.starts_with("https:") {
            "(remote)".to_string()
          } else if label.starts_with("data:") {
            "(data)".to_string()
          } else {
            "(local)".to_string()
          },
        );
        let text_edit = Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
          range: *range,
          new_text: label.clone(),
        }));
        Some(lsp::CompletionItem {
          label,
          kind: Some(lsp::CompletionItemKind::File),
          detail,
          sort_text: Some("1".to_string()),
          text_edit,
          ..Default::default()
        })
      } else {
        None
      }
    })
    .collect()
}

/// A structure that implements the visit trait to determine if the supplied
/// position falls within the module specifier of an import/export statement.
/// Once the module has been visited,
struct ImportLocator {
  pub maybe_range: Option<lsp::Range>,
  pub maybe_specifier: Option<String>,
  position: lsp::Position,
  source_map: Rc<SourceMap>,
}

impl ImportLocator {
  pub fn new(position: lsp::Position, source_map: Rc<SourceMap>) -> Self {
    Self {
      maybe_range: None,
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
        self.maybe_range = Some(lsp::Range {
          start: lsp::Position {
            line: (start.line - 1) as u32,
            character: (start.col_display + 1) as u32,
          },
          end: lsp::Position {
            line: (end.line - 1) as u32,
            character: (end.col_display - 1) as u32,
          },
        });
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
) -> Option<(String, lsp::Range)> {
  if let Ok(parsed_module) =
    analysis::parse_module(specifier, source, media_type)
  {
    let mut import_locator =
      ImportLocator::new(*position, parsed_module.source_map.clone());
    parsed_module
      .module
      .visit_with(&swc_ast::Invalid { span: DUMMY_SP }, &mut import_locator);
    if let (Some(specifier), Some(range)) =
      (import_locator.maybe_specifier, import_locator.maybe_range)
    {
      Some((specifier, range))
    } else {
      None
    }
  } else {
    None
  }
}

/// Converts a specifier into a relative specifier to the provided base
/// specifier as a string.  If a relative path cannot be found, then the
/// specifier is simply returned as a string.
///
/// ```
/// use deno_core::resolve_url;
///
/// let specifier = resolve_url("file:///a/b.ts").unwrap();
/// let base = resolve_url("file:///a/c/d.ts").unwrap();
/// assert_eq!(relative_specifier(&specifier, &base), "../b.ts");
/// ```
///
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
            #[allow(clippy::same_item_push)]
            for _ in segments_b {
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
  use crate::http_cache::HttpCache;
  use crate::lsp::analysis;
  use crate::lsp::documents::DocumentCache;
  use crate::lsp::sources::Sources;
  use crate::media_type::MediaType;
  use deno_core::resolve_url;
  use std::collections::HashMap;
  use std::path::Path;
  use tempfile::TempDir;

  fn mock_state_snapshot(
    fixtures: &[(&str, &str, i32)],
    source_fixtures: &[(&str, &str)],
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
    let http_cache = HttpCache::new(location);
    for (specifier, source) in source_fixtures {
      let specifier =
        resolve_url(specifier).expect("failed to create specifier");
      http_cache
        .set(&specifier, HashMap::default(), source.as_bytes())
        .expect("could not cache file");
      assert!(
        sources.get_source(&specifier).is_some(),
        "source could not be setup"
      );
    }
    language_server::StateSnapshot {
      documents,
      sources,
      ..Default::default()
    }
  }

  fn setup(
    documents: &[(&str, &str, i32)],
    sources: &[(&str, &str)],
  ) -> language_server::StateSnapshot {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let location = temp_dir.path().join("deps");
    mock_state_snapshot(documents, sources, &location)
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
      #[cfg(not(windows))]
      ("file:///a/b/c.ts", "file:///a/b/d.ts", "./c.ts"),
      #[cfg(not(windows))]
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
      Some((
        "".to_string(),
        lsp::Range {
          start: lsp::Position {
            line: 0,
            character: 20
          },
          end: lsp::Position {
            line: 0,
            character: 20
          }
        }
      ))
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
      Some((
        "https://".to_string(),
        lsp::Range {
          start: lsp::Position {
            line: 0,
            character: 20
          },
          end: lsp::Position {
            line: 0,
            character: 28
          }
        }
      ))
    );
  }

  #[test]
  fn test_get_local_completions() {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let fixtures = temp_dir.path().join("fixtures");
    std::fs::create_dir(&fixtures).expect("could not create");
    let dir_a = fixtures.join("a");
    std::fs::create_dir(&dir_a).expect("could not create");
    let dir_b = dir_a.join("b");
    std::fs::create_dir(&dir_b).expect("could not create");
    let file_c = dir_a.join("c.ts");
    std::fs::write(&file_c, b"").expect("could not create");
    let file_d = dir_b.join("d.ts");
    std::fs::write(&file_d, b"").expect("could not create");
    let file_e = dir_a.join("e.txt");
    std::fs::write(&file_e, b"").expect("could not create");
    let file_f = dir_a.join("f.mjs");
    std::fs::write(&file_f, b"").expect("could not create");
    let specifier =
      ModuleSpecifier::from_file_path(file_c).expect("could not create");
    let actual = get_local_completions(
      &specifier,
      "./",
      &lsp::Range {
        start: lsp::Position {
          line: 0,
          character: 20,
        },
        end: lsp::Position {
          line: 0,
          character: 22,
        },
      },
    );
    assert!(actual.is_some());
    let actual = actual.unwrap();
    assert_eq!(actual.len(), 2);
    for item in actual {
      match item.text_edit {
        Some(lsp::CompletionTextEdit::Edit(text_edit)) => {
          assert!(
            text_edit.new_text == "./f.mjs" || text_edit.new_text == "./b"
          );
        }
        _ => unreachable!(),
      }
    }
  }

  #[tokio::test]
  async fn test_get_import_completions() {
    let specifier = resolve_url("file:///a/b/c.ts").unwrap();
    let position = lsp::Position {
      line: 0,
      character: 21,
    };
    let state_snapshot = setup(
      &[
        ("file:///a/b/c.ts", "import * as d from \"h\"", 1),
        ("file:///a/c.ts", r#""#, 1),
      ],
      &[("https://deno.land/x/a/b/c.ts", "console.log(1);\n")],
    );
    let actual =
      get_import_completions(&specifier, &position, &state_snapshot).await;
    assert_eq!(
      actual,
      Some(lsp::CompletionResponse::List(lsp::CompletionList {
        is_incomplete: false,
        items: vec![lsp::CompletionItem {
          label: "https://deno.land/x/a/b/c.ts".to_string(),
          kind: Some(lsp::CompletionItemKind::File),
          detail: Some("(remote)".to_string()),
          sort_text: Some("1".to_string()),
          text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
            range: lsp::Range {
              start: lsp::Position {
                line: 0,
                character: 20
              },
              end: lsp::Position {
                line: 0,
                character: 21,
              }
            },
            new_text: "https://deno.land/x/a/b/c.ts".to_string(),
          })),
          ..Default::default()
        }]
      }))
    );
  }
}
