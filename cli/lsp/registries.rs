// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::language_server;
use super::path_to_regex::parse;
use super::path_to_regex::string_to_regex;
use super::path_to_regex::Compiler;
use super::path_to_regex::Key;
use super::path_to_regex::MatchResult;
use super::path_to_regex::Matcher;
use super::path_to_regex::StringOrNumber;
use super::path_to_regex::StringOrVec;
use super::path_to_regex::Token;

use crate::deno_dir;
use crate::file_fetcher::CacheSetting;
use crate::file_fetcher::FileFetcher;
use crate::http_cache::HttpCache;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use log::error;
use lspower::lsp;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

const CONFIG_PATH: &str = "/.well-known/deno-import-intellisense.json";
const COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
  .add(b' ')
  .add(b'"')
  .add(b'#')
  .add(b'<')
  .add(b'>')
  .add(b'?')
  .add(b'`')
  .add(b'{')
  .add(b'}')
  .add(b'/')
  .add(b':')
  .add(b';')
  .add(b'=')
  .add(b'@')
  .add(b'[')
  .add(b'\\')
  .add(b']')
  .add(b'^')
  .add(b'|')
  .add(b'$')
  .add(b'&')
  .add(b'+')
  .add(b',');

lazy_static::lazy_static! {
  static ref REPLACEMENT_VARIABLE_RE: Regex =
    Regex::new(r"\$\{\{?(\w+)\}?\}").unwrap();
}

fn base_url(url: &Url) -> Result<Url, AnyError> {
  let mut url = url.clone();
  match url.path_segments_mut() {
    Ok(mut path) => {
      path.clear();
    }
    Err(_) => {
      return Err(anyhow!("Url cannot be used as base"));
    }
  }
  url.set_query(None);
  Ok(url)
}

#[derive(Debug)]
enum CompletorType {
  Literal(String),
  Key(Key),
}

fn get_completor_type(
  offset: usize,
  tokens: &[Token],
  match_result: &MatchResult,
) -> Option<CompletorType> {
  let mut len = 0_usize;
  for token in tokens {
    match token {
      Token::String(s) => {
        len += s.chars().count();
        if offset < len {
          return Some(CompletorType::Literal(s.clone()));
        }
      }
      Token::Key(k) => {
        if let Some(prefix) = &k.prefix {
          len += prefix.chars().count();
        }
        if offset < len {
          return None;
        }
        if let StringOrNumber::String(name) = &k.name {
          let value = match_result
            .get(name)
            .map(|s| s.to_string(Some(&k)))
            .unwrap_or_default();
          len += value.chars().count();
          if offset <= len {
            return Some(CompletorType::Key(k.clone()));
          }
        }
        if let Some(suffix) = &k.suffix {
          len += suffix.chars().count();
          if offset <= len {
            return Some(CompletorType::Literal(suffix.clone()));
          }
        }
      }
    }
  }

  None
}

fn get_completion_endpoint(
  url: &str,
  tokens: &[Token],
  match_result: &MatchResult,
) -> Result<ModuleSpecifier, AnyError> {
  let mut url_str = url.to_string();
  for (key, value) in match_result.params.iter() {
    if let StringOrNumber::String(name) = key {
      let maybe_key = tokens.iter().find_map(|t| match t {
        Token::Key(k) if k.name == *key => Some(k),
        _ => None,
      });
      url_str =
        url_str.replace(&format!("${{{}}}", name), &value.to_string(maybe_key));
      url_str = url_str.replace(
        &format!("${{{{{}}}}}", name),
        &percent_encoding::percent_encode(
          value.to_string(maybe_key).as_bytes(),
          COMPONENT,
        )
        .to_string(),
      );
    }
  }
  resolve_url(&url_str).map_err(|err| err.into())
}

fn parse_replacement_variables<S: AsRef<str>>(s: S) -> HashSet<String> {
  REPLACEMENT_VARIABLE_RE
    .captures_iter(s.as_ref())
    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
    .collect()
}

fn validate_config(config: &RegistryConfigurationJson) -> Result<(), AnyError> {
  if config.version != 1 {
    return Err(anyhow!(
      "Invalid registry configuration. Expected version 1 got {}.",
      config.version
    ));
  }
  for registry in &config.registries {
    let (_, keys) = string_to_regex(&registry.schema, None)?;
    let key_names: HashSet<String> = keys.map_or_else(HashSet::new, |keys| {
      keys
        .iter()
        .filter_map(|k| {
          if let StringOrNumber::String(s) = &k.name {
            Some(s.clone())
          } else {
            None
          }
        })
        .collect()
    });
    let mut variable_names = HashSet::<String>::new();
    for variable in &registry.variables {
      variable_names.insert(variable.key.clone());
      if !key_names.contains(&variable.key) {
        return Err(anyhow!("Invalid registry configuration. Variable \"{}\" is not present in the schema: \"{}\".", variable.key, registry.schema));
      }
      for url_var in &parse_replacement_variables(&variable.url) {
        if !key_names.contains(url_var) {
          return Err(anyhow!("Invalid registry configuration. Variable url \"{}\" is not present in the schema: \"{}\".", url_var, registry.schema));
        }
      }
    }
    for key_name in &key_names {
      if !variable_names.contains(key_name) {
        return Err(anyhow!("Invalid registry configuration. Schema contains key \"{}\" which does not have a defined variable.", key_name));
      }
    }
  }

  Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryConfigurationVariable {
  key: String,
  url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryConfiguration {
  schema: String,
  variables: Vec<RegistryConfigurationVariable>,
}

#[derive(Debug, Deserialize)]
struct RegistryConfigurationJson {
  version: u32,
  registries: Vec<RegistryConfiguration>,
}

#[derive(Debug, Clone)]
pub struct ModuleRegistry {
  origins: HashMap<ModuleSpecifier, Vec<RegistryConfiguration>>,
  file_fetcher: FileFetcher,
}

impl Default for ModuleRegistry {
  fn default() -> Self {
    let custom_root = std::env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(custom_root).unwrap();
    let location = dir.root.join("registries");
    let http_cache = HttpCache::new(&location);
    let cache_setting = CacheSetting::Use;
    let file_fetcher =
      FileFetcher::new(http_cache, cache_setting, true, None).unwrap();

    Self {
      origins: HashMap::new(),
      file_fetcher,
    }
  }
}

impl ModuleRegistry {
  pub fn new(location: &Path, reload: bool) -> Self {
    let http_cache = HttpCache::new(location);
    let cache_setting = if reload {
      CacheSetting::ReloadAll
    } else {
      CacheSetting::Use
    };
    let file_fetcher = FileFetcher::new(http_cache, cache_setting, true, None)
      .context("Error creating file fetcher in module registry.")
      .unwrap();

    Self {
      origins: HashMap::new(),
      file_fetcher,
    }
  }

  pub async fn disable(&mut self, origin: &str) -> Result<(), AnyError> {
    let origin = base_url(&Url::parse(origin)?)?;
    self.origins.remove(&origin);
    Ok(())
  }

  async fn fetch_config(
    &self,
    origin: &ModuleSpecifier,
  ) -> Result<Vec<RegistryConfiguration>, AnyError> {
    let specifier = origin.join(CONFIG_PATH)?;
    let file = self
      .file_fetcher
      .fetch(&specifier, &Permissions::allow_all())
      .await?;
    let config: RegistryConfigurationJson = serde_json::from_str(&file.source)?;
    validate_config(&config)?;
    Ok(config.registries)
  }

  pub async fn enable(&mut self, origin: &str) -> Result<(), AnyError> {
    let origin = base_url(&Url::parse(origin)?)?;
    #[allow(clippy::map_entry)]
    // we can't use entry().or_insert_with() because we can't use async closures
    if !self.origins.contains_key(&origin) {
      let configs = self.fetch_config(&origin).await?;
      self.origins.insert(origin, configs);
    }

    Ok(())
  }

  pub async fn get_completions(
    &self,
    current_specifier: &str,
    offset: usize,
    range: &lsp::Range,
    state_snapshot: &language_server::StateSnapshot,
  ) -> Option<Vec<lsp::CompletionItem>> {
    if let Ok(specifier) = Url::parse(current_specifier) {
      let origin = base_url(&specifier).ok()?;
      let origin_len = specifier[..Position::BeforePath].chars().count();
      if offset >= origin_len {
        if let Some(registries) = self.origins.get(&origin) {
          let path = &specifier[Position::BeforePath..];
          let path_offset = offset - origin_len;
          let mut completions = HashMap::<String, lsp::CompletionItem>::new();
          for registry in registries {
            let tokens = parse(&registry.schema, None)
              .map_err(|e| {
                error!(
                  "Error parsing registry schema for origin \"{}\". {}",
                  origin, e
                );
              })
              .ok()?;
            let mut i = tokens.len();
            let last_key_name =
              StringOrNumber::String(tokens.iter().last().map_or_else(
                || "".to_string(),
                |t| {
                  if let Token::Key(key) = t {
                    if let StringOrNumber::String(s) = &key.name {
                      return s.clone();
                    }
                  }
                  "".to_string()
                },
              ));
            loop {
              let matcher = Matcher::new(&tokens[..i], None)
                .map_err(|e| {
                  error!(
                    "Error creating matcher for schema for origin \"{}\". {}",
                    origin, e
                  );
                })
                .ok()?;
              if let Some(match_result) = matcher.matches(path) {
                let completor_type =
                  get_completor_type(path_offset, &tokens, &match_result);
                match completor_type {
                  Some(CompletorType::Literal(s)) => {
                    let label = if s.starts_with('/') {
                      s[0..].to_string()
                    } else {
                      s.to_string()
                    };
                    let full_text = format!(
                      "{}{}{}",
                      &current_specifier[..offset],
                      s,
                      &current_specifier[offset..]
                    );
                    let text_edit =
                      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                        range: *range,
                        new_text: full_text.clone(),
                      }));
                    let filter_text = Some(full_text);
                    completions.insert(
                      s,
                      lsp::CompletionItem {
                        label,
                        kind: Some(lsp::CompletionItemKind::Folder),
                        filter_text,
                        sort_text: Some("1".to_string()),
                        text_edit,
                        ..Default::default()
                      },
                    );
                  }
                  Some(CompletorType::Key(k)) => {
                    let maybe_url = registry.variables.iter().find_map(|v| {
                      if k.name == StringOrNumber::String(v.key.clone()) {
                        Some(v.url.as_str())
                      } else {
                        None
                      }
                    });
                    if let Some(url) = maybe_url {
                      if let Some(items) = self
                        .get_variable_items(url, &tokens, &match_result)
                        .await
                      {
                        let compiler = Compiler::new(&tokens[..i], None);
                        for item in items {
                          let label = item.clone();
                          let kind = if k.name == last_key_name {
                            Some(lsp::CompletionItemKind::File)
                          } else {
                            Some(lsp::CompletionItemKind::Folder)
                          };
                          let mut params = match_result.params.clone();
                          params.insert(
                            k.name.clone(),
                            StringOrVec::from_str(&item, &k),
                          );
                          let path =
                            compiler.to_path(&params).unwrap_or_default();
                          let mut item_specifier = origin.clone();
                          item_specifier.set_path(&path);
                          let full_text = item_specifier.as_str();
                          let text_edit = Some(lsp::CompletionTextEdit::Edit(
                            lsp::TextEdit {
                              range: *range,
                              new_text: full_text.to_string(),
                            },
                          ));
                          let command = if k.name == last_key_name
                            && !state_snapshot
                              .sources
                              .contains_key(&item_specifier)
                          {
                            Some(lsp::Command {
                              title: "".to_string(),
                              command: "deno.cache".to_string(),
                              arguments: Some(vec![json!([item_specifier])]),
                            })
                          } else {
                            None
                          };
                          let detail = Some(format!("({})", k.name));
                          let filter_text = Some(full_text.to_string());
                          completions.insert(
                            item,
                            lsp::CompletionItem {
                              label,
                              kind,
                              detail,
                              filter_text,
                              sort_text: Some("1".to_string()),
                              text_edit,
                              command,
                              ..Default::default()
                            },
                          );
                        }
                      }
                    }
                  }
                  None => (),
                }
                break;
              }
              i -= 1;
              if i == 0 {
                if let Token::String(s) = &tokens[i] {
                  if s.starts_with(path) {
                    let label = s.to_string();
                    let kind = Some(lsp::CompletionItemKind::Folder);
                    let full_text = format!(
                      "{}{}{}",
                      &current_specifier[..offset],
                      s.replacen(path, "", 1),
                      &current_specifier[offset..]
                    );
                    let text_edit =
                      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                        range: *range,
                        new_text: full_text.clone(),
                      }));
                    let filter_text = Some(full_text);
                    completions.insert(
                      s.to_string(),
                      lsp::CompletionItem {
                        label,
                        kind,
                        filter_text,
                        sort_text: Some("1".to_string()),
                        text_edit,
                        ..Default::default()
                      },
                    );
                  }
                }
                break;
              }
            }
          }
          return if completions.is_empty() {
            None
          } else {
            Some(completions.into_iter().map(|(_, i)| i).collect())
          };
        }
      }
    }
    if !current_specifier.is_empty() {
      Some(
        self
          .origins
          .keys()
          .filter_map(|k| {
            let mut origin = k.as_str().to_string();
            if origin.ends_with('/') {
              origin.pop();
            }
            if origin.starts_with(current_specifier) {
              let text_edit =
                Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                  range: *range,
                  new_text: origin.clone(),
                }));
              Some(lsp::CompletionItem {
                label: origin,
                kind: Some(lsp::CompletionItemKind::Folder),
                detail: Some("(registry)".to_string()),
                sort_text: Some("1".to_string()),
                text_edit,
                ..Default::default()
              })
            } else {
              None
            }
          })
          .collect(),
      )
    } else {
      None
    }
  }

  async fn get_variable_items(
    &self,
    url: &str,
    tokens: &[Token],
    match_result: &MatchResult,
  ) -> Option<Vec<String>> {
    let specifier = get_completion_endpoint(url, tokens, match_result)
      .map_err(|err| {
        error!("Internal error mapping endpoint \"{}\". {}", url, err);
      })
      .ok()?;
    let file = self
      .file_fetcher
      .fetch(&specifier, &Permissions::allow_all())
      .await
      .map_err(|err| {
        error!(
          "Internal error fetching endpoint \"{}\". {}",
          specifier, err
        );
      })
      .ok()?;
    let items: Vec<String> = serde_json::from_str(&file.source)
      .map_err(|err| {
        error!(
          "Error parsing response from endpoint \"{}\". {}",
          specifier, err
        );
      })
      .ok()?;
    Some(items)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::lsp::documents::DocumentCache;
  use crate::lsp::sources::Sources;
  use tempfile::TempDir;

  fn mock_state_snapshot(
    source_fixtures: &[(&str, &str)],
    location: &Path,
  ) -> language_server::StateSnapshot {
    let documents = DocumentCache::default();
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

  fn setup(sources: &[(&str, &str)]) -> language_server::StateSnapshot {
    let temp_dir = TempDir::new().expect("could not create temp dir");
    let location = temp_dir.path().join("deps");
    mock_state_snapshot(sources, &location)
  }

  #[tokio::test]
  async fn test_registry_completions_origin_match() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new().expect("could not create tmp");
    let location = temp_dir.path().join("registries");
    let mut module_registry = ModuleRegistry::new(&location, true);
    module_registry
      .enable("http://localhost:4545/")
      .await
      .expect("could not enable");
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
    let state_snapshot = setup(&[]);
    let completions = module_registry
      .get_completions("h", 1, &range, &state_snapshot)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].label, "http://localhost:4545/");
    assert_eq!(
      completions[0].text_edit,
      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
        range,
        new_text: "http://localhost:4545/".to_string()
      }))
    );
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 36,
      },
    };
    let completions = module_registry
      .get_completions("http://localhost", 16, &range, &state_snapshot)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].label, "http://localhost:4545/");
    assert_eq!(
      completions[0].text_edit,
      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
        range,
        new_text: "http://localhost:4545/".to_string()
      }))
    );
  }

  #[tokio::test]
  async fn test_registry_completions() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new().expect("could not create tmp");
    let location = temp_dir.path().join("registries");
    let mut module_registry = ModuleRegistry::new(&location, true);
    module_registry
      .enable("http://localhost:4545/")
      .await
      .expect("could not enable");
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 42,
      },
    };
    let state_snapshot = setup(&[]);
    let completions = module_registry
      .get_completions("http://localhost:4545/", 22, &range, &state_snapshot)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].label, "/x");
    assert_eq!(
      completions[0].text_edit,
      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
        range,
        new_text: "http://localhost:4545/x".to_string()
      }))
    );
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 44,
      },
    };
    let completions = module_registry
      .get_completions("http://localhost:4545/x/", 24, &range, &state_snapshot)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.len(), 2);
    assert!(
      completions[0].label == "a".to_string()
        || completions[0].label == "b".to_string()
    );
    assert!(
      completions[1].label == "a".to_string()
        || completions[1].label == "b".to_string()
    );
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 46,
      },
    };
    let completions = module_registry
      .get_completions(
        "http://localhost:4545/x/a@",
        26,
        &range,
        &state_snapshot,
      )
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.len(), 3);
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 53,
      },
    };
    let completions = module_registry
      .get_completions(
        "http://localhost:4545/x/a@v1.0.0/",
        33,
        &range,
        &state_snapshot,
      )
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].detail, Some("(path)".to_string()));
    assert_eq!(completions[0].kind, Some(lsp::CompletionItemKind::File));
    assert!(completions[0].command.is_some());
    assert_eq!(completions[1].detail, Some("(path)".to_string()));
    assert_eq!(completions[0].kind, Some(lsp::CompletionItemKind::File));
    assert!(completions[1].command.is_some());
  }

  #[test]
  fn test_parse_replacement_variables() {
    let actual = parse_replacement_variables(
      "https://deno.land/_vsc1/modules/${module}/v/${{version}}",
    );
    assert_eq!(actual.iter().count(), 2);
    assert!(actual.contains("module"));
    assert!(actual.contains("version"));
  }
}
