// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::client::Client;
use super::config::ConfigSnapshot;
use super::documents::Documents;
use super::lsp_custom;
use super::registries::ModuleRegistry;
use super::tsc;

use crate::fs_util::is_supported_ext;
use crate::fs_util::specifier_to_file_path;

use deno_ast::LineAndColumnIndex;
use deno_ast::SourceTextInfo;
use deno_core::normalize_path;
use deno_core::resolve_path;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::url::Position;
use deno_core::ModuleSpecifier;
use import_map::ImportMap;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

static FILE_PROTO_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"^file:/{2}(?:/[A-Za-z]:)?"#).unwrap());

const CURRENT_PATH: &str = ".";
const PARENT_PATH: &str = "..";
const LOCAL_PATHS: &[&str] = &[CURRENT_PATH, PARENT_PATH];
pub(crate) const IMPORT_COMMIT_CHARS: &[&str] = &["\"", "'"];

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemData {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub documentation: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tsc: Option<tsc::CompletionItemData>,
}

/// Check if the origin can be auto-configured for completions, and if so, send
/// a notification to the client.
async fn check_auto_config_registry(
  url_str: &str,
  config: &ConfigSnapshot,
  client: Client,
  module_registries: &ModuleRegistry,
) {
  // check to see if auto discovery is enabled
  if config.settings.workspace.suggest.imports.auto_discover {
    if let Ok(specifier) = resolve_url(url_str) {
      let scheme = specifier.scheme();
      let path = &specifier[Position::BeforePath..];
      if scheme.starts_with("http")
        && !path.is_empty()
        && url_str.ends_with(path)
      {
        // check to see if this origin is already explicitly set
        let in_config =
          config.settings.workspace.suggest.imports.hosts.iter().any(
            |(h, _)| {
              resolve_url(h).map(|u| u.origin()) == Ok(specifier.origin())
            },
          );
        // if it isn't in the configuration, we will check to see if it supports
        // suggestions and send a notification to the client.
        if !in_config {
          let origin = specifier.origin().ascii_serialization();
          let suggestions =
            module_registries.check_origin(&origin).await.is_ok();
          // we are only sending registry state when enabled now, but changing
          // the custom notification would make older versions of the plugin
          // incompatible.
          // TODO(@kitsonk) clean up protocol when doing v2 of suggestions
          if suggestions {
            client
              .send_registry_state_notification(
                lsp_custom::RegistryStateNotificationParams {
                  origin,
                  suggestions,
                },
              )
              .await;
          }
        }
      }
    }
  }
}

/// Ranges from the graph for specifiers include the leading and maybe trailing quote,
/// which we want to ignore when replacing text.
fn to_narrow_lsp_range(
  text_info: &SourceTextInfo,
  range: &deno_graph::Range,
) -> lsp::Range {
  let end_byte_index = text_info
    .loc_to_source_pos(LineAndColumnIndex {
      line_index: range.end.line,
      column_index: range.end.character,
    })
    .as_byte_index(text_info.range().start);
  let start_byte_index = text_info
    .loc_to_source_pos(LineAndColumnIndex {
      line_index: range.start.line,
      column_index: range.start.character,
    })
    .as_byte_index(text_info.range().start);
  let text_bytes = text_info.text_str().as_bytes();
  let is_empty = end_byte_index - 1 == start_byte_index;
  let has_trailing_quote =
    !is_empty && matches!(text_bytes[end_byte_index - 1], b'"' | b'\'');
  lsp::Range {
    start: lsp::Position {
      line: range.start.line as u32,
      // skip the leading quote
      character: (range.start.character + 1) as u32,
    },
    end: lsp::Position {
      line: range.end.line as u32,
      character: if has_trailing_quote {
        range.end.character - 1 // do not include it
      } else {
        range.end.character
      } as u32,
    },
  }
}

/// Given a specifier, a position, and a snapshot, optionally return a
/// completion response, which will be valid import completions for the specific
/// context.
pub async fn get_import_completions(
  specifier: &ModuleSpecifier,
  position: &lsp::Position,
  config: &ConfigSnapshot,
  client: Client,
  module_registries: &ModuleRegistry,
  documents: &Documents,
  maybe_import_map: Option<Arc<ImportMap>>,
) -> Option<lsp::CompletionResponse> {
  let document = documents.get(specifier)?;
  let (text, _, range) = document.get_maybe_dependency(position)?;
  let range = to_narrow_lsp_range(&document.text_info(), &range);
  if let Some(completion_list) = get_import_map_completions(
    specifier,
    &text,
    &range,
    maybe_import_map.clone(),
    documents,
  ) {
    // completions for import map specifiers
    Some(lsp::CompletionResponse::List(completion_list))
  } else if text.starts_with("./") || text.starts_with("../") {
    // completions for local relative modules
    Some(lsp::CompletionResponse::List(lsp::CompletionList {
      is_incomplete: false,
      items: get_local_completions(specifier, &text, &range)?,
    }))
  } else if !text.is_empty() {
    // completion of modules from a module registry or cache
    check_auto_config_registry(&text, config, client, module_registries).await;
    let offset = if position.character > range.start.character {
      (position.character - range.start.character) as usize
    } else {
      0
    };
    let maybe_list = module_registries
      .get_completions(&text, offset, &range, |specifier| {
        documents.exists(specifier)
      })
      .await;
    let list = maybe_list.unwrap_or_else(|| lsp::CompletionList {
      items: get_workspace_completions(specifier, &text, &range, documents),
      is_incomplete: false,
    });
    Some(lsp::CompletionResponse::List(list))
  } else {
    // the import specifier is empty, so provide all possible specifiers we are
    // aware of
    let mut items: Vec<lsp::CompletionItem> = LOCAL_PATHS
      .iter()
      .map(|s| lsp::CompletionItem {
        label: s.to_string(),
        kind: Some(lsp::CompletionItemKind::FOLDER),
        detail: Some("(local)".to_string()),
        sort_text: Some("1".to_string()),
        insert_text: Some(s.to_string()),
        commit_characters: Some(
          IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
        ),
        ..Default::default()
      })
      .collect();
    let mut is_incomplete = false;
    if let Some(import_map) = maybe_import_map {
      items.extend(get_base_import_map_completions(import_map.as_ref()));
    }
    if let Some(origin_items) =
      module_registries.get_origin_completions(&text, &range)
    {
      is_incomplete = origin_items.is_incomplete;
      items.extend(origin_items.items);
    }
    Some(lsp::CompletionResponse::List(lsp::CompletionList {
      is_incomplete,
      items,
    }))
  }
}

/// When the specifier is an empty string, return all the keys from the import
/// map as completion items.
fn get_base_import_map_completions(
  import_map: &ImportMap,
) -> Vec<lsp::CompletionItem> {
  import_map
    .imports()
    .keys()
    .map(|key| {
      // for some strange reason, keys that start with `/` get stored in the
      // import map as `file:///`, and so when we pull the keys out, we need to
      // change the behavior
      let mut label = if key.starts_with("file://") {
        FILE_PROTO_RE.replace(key, "").to_string()
      } else {
        key.to_string()
      };
      let kind = if key.ends_with('/') {
        label.pop();
        Some(lsp::CompletionItemKind::FOLDER)
      } else {
        Some(lsp::CompletionItemKind::FILE)
      };
      lsp::CompletionItem {
        label: label.clone(),
        kind,
        detail: Some("(import map)".to_string()),
        sort_text: Some(label.clone()),
        insert_text: Some(label),
        commit_characters: Some(
          IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
        ),
        ..Default::default()
      }
    })
    .collect()
}

/// Given an existing specifier, return any completions that could apply derived
/// from the import map. There are two main type of import map keys, those that
/// a literal, which don't end in `/`, which expects a one for one replacement
/// of specifier to specifier, and then those that end in `/` which indicates
/// that the path post the `/` should be appended to resolved specifier. This
/// handles both cases, pulling any completions from the workspace completions.
fn get_import_map_completions(
  specifier: &ModuleSpecifier,
  text: &str,
  range: &lsp::Range,
  maybe_import_map: Option<Arc<ImportMap>>,
  documents: &Documents,
) -> Option<lsp::CompletionList> {
  if !text.is_empty() {
    if let Some(import_map) = maybe_import_map {
      let mut items = Vec::new();
      for key in import_map.imports().keys() {
        // for some reason, the import_map stores keys that begin with `/` as
        // `file:///` in its index, so we have to reverse that here
        let key = if key.starts_with("file://") {
          FILE_PROTO_RE.replace(key, "").to_string()
        } else {
          key.to_string()
        };
        if text.starts_with(&key) && key.ends_with('/') {
          if let Ok(resolved) = import_map.resolve(&key, specifier) {
            let resolved = resolved.to_string();
            let workspace_items: Vec<lsp::CompletionItem> = documents
              .documents(false, true)
              .into_iter()
              .filter_map(|d| {
                let specifier_str = d.specifier().to_string();
                let new_text = specifier_str.replace(&resolved, &key);
                if specifier_str.starts_with(&resolved) {
                  let label = specifier_str.replace(&resolved, "");
                  let text_edit =
                    Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                      range: *range,
                      new_text: new_text.clone(),
                    }));
                  Some(lsp::CompletionItem {
                    label,
                    kind: Some(lsp::CompletionItemKind::MODULE),
                    detail: Some("(import map)".to_string()),
                    sort_text: Some("1".to_string()),
                    filter_text: Some(new_text),
                    text_edit,
                    commit_characters: Some(
                      IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
                    ),
                    ..Default::default()
                  })
                } else {
                  None
                }
              })
              .collect();
            items.extend(workspace_items);
          }
        } else if key.starts_with(text) && text != key {
          let mut label = key.to_string();
          let kind = if key.ends_with('/') {
            label.pop();
            Some(lsp::CompletionItemKind::FOLDER)
          } else {
            Some(lsp::CompletionItemKind::MODULE)
          };
          let text_edit = Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
            range: *range,
            new_text: label.clone(),
          }));
          items.push(lsp::CompletionItem {
            label: label.clone(),
            kind,
            detail: Some("(import map)".to_string()),
            sort_text: Some("1".to_string()),
            text_edit,
            commit_characters: Some(
              IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
            ),
            ..Default::default()
          });
        }
        if !items.is_empty() {
          return Some(lsp::CompletionList {
            items,
            is_incomplete: false,
          });
        }
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

  let mut base_path = specifier_to_file_path(base).ok()?;
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
              kind: Some(lsp::CompletionItemKind::FOLDER),
              filter_text,
              sort_text: Some("1".to_string()),
              text_edit,
              commit_characters: Some(
                IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
              ),
              ..Default::default()
            }),
            Ok(file_type) if file_type.is_file() => {
              if is_supported_ext(&de.path()) {
                Some(lsp::CompletionItem {
                  label,
                  kind: Some(lsp::CompletionItemKind::FILE),
                  detail: Some("(local)".to_string()),
                  filter_text,
                  sort_text: Some("1".to_string()),
                  text_edit,
                  commit_characters: Some(
                    IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
                  ),
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
  documents: &Documents,
) -> Vec<lsp::CompletionItem> {
  let workspace_specifiers = documents
    .documents(false, true)
    .into_iter()
    .map(|d| d.specifier().clone())
    .collect();
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
          kind: Some(lsp::CompletionItemKind::FILE),
          detail,
          sort_text: Some("1".to_string()),
          text_edit,
          commit_characters: Some(
            IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
          ),
          ..Default::default()
        })
      } else {
        None
      }
    })
    .collect()
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
pub fn relative_specifier(
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
      specifier_to_file_path(specifier)
        .unwrap()
        .to_string_lossy()
        .into()
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
        format!("./{}{}", last_a, &specifier[Position::AfterPath..])
      } else {
        parts.push(last_a);
        format!("{}{}", parts.join("/"), &specifier[Position::AfterPath..])
      }
    } else {
      specifier[Position::BeforePath..].into()
    }
  } else {
    specifier[Position::BeforePath..].into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::http_cache::HttpCache;
  use crate::lsp::documents::Documents;
  use crate::lsp::documents::LanguageId;
  use deno_core::resolve_url;
  use deno_graph::Range;
  use std::collections::HashMap;
  use std::path::Path;
  use test_util::TempDir;

  fn mock_documents(
    fixtures: &[(&str, &str, i32, LanguageId)],
    source_fixtures: &[(&str, &str)],
    location: &Path,
  ) -> Documents {
    let mut documents = Documents::new(location);
    for (specifier, source, version, language_id) in fixtures {
      let specifier =
        resolve_url(specifier).expect("failed to create specifier");
      documents.open(
        specifier.clone(),
        *version,
        language_id.clone(),
        (*source).into(),
      );
    }
    let http_cache = HttpCache::new(location);
    for (specifier, source) in source_fixtures {
      let specifier =
        resolve_url(specifier).expect("failed to create specifier");
      http_cache
        .set(&specifier, HashMap::default(), source.as_bytes())
        .expect("could not cache file");
      assert!(
        documents.get(&specifier).is_some(),
        "source could not be setup"
      );
    }
    documents
  }

  fn setup(
    temp_dir: &TempDir,
    documents: &[(&str, &str, i32, LanguageId)],
    sources: &[(&str, &str)],
  ) -> Documents {
    let location = temp_dir.path().join("deps");
    mock_documents(documents, sources, &location)
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
  fn test_get_local_completions() {
    let temp_dir = TempDir::new();
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
  async fn test_get_workspace_completions() {
    let specifier = resolve_url("file:///a/b/c.ts").unwrap();
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 21,
      },
    };
    let temp_dir = TempDir::new();
    let documents = setup(
      &temp_dir,
      &[
        (
          "file:///a/b/c.ts",
          "import * as d from \"h\"",
          1,
          LanguageId::TypeScript,
        ),
        ("file:///a/c.ts", r#""#, 1, LanguageId::TypeScript),
      ],
      &[("https://deno.land/x/a/b/c.ts", "console.log(1);\n")],
    );
    let actual = get_workspace_completions(&specifier, "h", &range, &documents);
    assert_eq!(
      actual,
      vec![lsp::CompletionItem {
        label: "https://deno.land/x/a/b/c.ts".to_string(),
        kind: Some(lsp::CompletionItemKind::FILE),
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
        commit_characters: Some(
          IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect()
        ),
        ..Default::default()
      }]
    );
  }

  #[test]
  fn test_to_narrow_lsp_range() {
    let text_info = SourceTextInfo::from_string(r#""te""#.to_string());
    let range = to_narrow_lsp_range(
      &text_info,
      &Range {
        specifier: ModuleSpecifier::parse("https://deno.land").unwrap(),
        start: deno_graph::Position {
          line: 0,
          character: 0,
        },
        end: deno_graph::Position {
          line: 0,
          character: text_info.text_str().chars().count(),
        },
      },
    );
    assert_eq!(range.start.character, 1);
    assert_eq!(
      range.end.character,
      (text_info.text_str().chars().count() - 1) as u32
    );
  }

  #[test]
  fn test_to_narrow_lsp_range_no_trailing_quote() {
    let text_info = SourceTextInfo::from_string(r#""te"#.to_string());
    let range = to_narrow_lsp_range(
      &text_info,
      &Range {
        specifier: ModuleSpecifier::parse("https://deno.land").unwrap(),
        start: deno_graph::Position {
          line: 0,
          character: 0,
        },
        end: deno_graph::Position {
          line: 0,
          character: text_info.text_str().chars().count(),
        },
      },
    );
    assert_eq!(range.start.character, 1);
    assert_eq!(
      range.end.character,
      text_info.text_str().chars().count() as u32
    );
  }
}
