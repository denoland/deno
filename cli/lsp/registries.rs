// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::completions::IMPORT_COMMIT_CHARS;
use super::logging::lsp_log;
use super::path_to_regex::parse;
use super::path_to_regex::string_to_regex;
use super::path_to_regex::Compiler;
use super::path_to_regex::Key;
use super::path_to_regex::MatchResult;
use super::path_to_regex::Matcher;
use super::path_to_regex::StringOrNumber;
use super::path_to_regex::StringOrVec;
use super::path_to_regex::Token;

use crate::args::CacheSetting;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::file_fetcher::FetchOptions;
use crate::file_fetcher::FetchPermissionsOptionRef;
use crate::file_fetcher::FileFetcher;
use crate::http_util::HttpClientProvider;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::ParseError;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::Dependency;
use log::error;
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

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

const REGISTRY_IMPORT_COMMIT_CHARS: &[&str] = &["\"", "'"];

static REPLACEMENT_VARIABLE_RE: Lazy<regex::Regex> =
  lazy_regex::lazy_regex!(r"\$\{\{?(\w+)\}?\}");

fn base_url(url: &Url) -> String {
  url.origin().ascii_serialization()
}

#[derive(Debug)]
enum CompletionType {
  Literal(String),
  Key {
    key: Key,
    prefix: Option<String>,
    index: usize,
  },
}

/// Determine if a completion at a given offset is a string literal or a key/
/// variable.
fn get_completion_type(
  char_offset: usize,
  tokens: &[Token],
  match_result: &MatchResult,
) -> Option<CompletionType> {
  let mut char_count = 0_usize;
  for (index, token) in tokens.iter().enumerate() {
    match token {
      Token::String(s) => {
        char_count += s.chars().count();
        if char_offset < char_count {
          return Some(CompletionType::Literal(s.clone()));
        }
      }
      Token::Key(k) => {
        if let Some(prefix) = &k.prefix {
          char_count += prefix.chars().count();
          if char_offset < char_count {
            return Some(CompletionType::Key {
              key: k.clone(),
              prefix: Some(prefix.clone()),
              index,
            });
          }
        }
        if char_offset < char_count {
          return None;
        }
        if let StringOrNumber::String(name) = &k.name {
          let value = match_result
            .get(name)
            .map(|s| s.to_string(Some(k), false))
            .unwrap_or_default();
          char_count += value.chars().count();
          if char_offset <= char_count {
            return Some(CompletionType::Key {
              key: k.clone(),
              prefix: None,
              index,
            });
          }
        }
        if let Some(suffix) = &k.suffix {
          char_count += suffix.chars().count();
          if char_offset <= char_count {
            return Some(CompletionType::Literal(suffix.clone()));
          }
        }
      }
    }
  }

  None
}

/// Generate a data value for a completion item that will instruct the client to
/// resolve the completion item to obtain further information, in this case, the
/// details/documentation endpoint for the item if it exists in the registry
/// configuration
fn get_data(
  registry: &RegistryConfiguration,
  base: &ModuleSpecifier,
  variable: &Key,
  value: &str,
) -> Option<Value> {
  let url = registry.get_documentation_url_for_key(variable)?;
  get_endpoint(url, base, variable, Some(value))
    .ok()
    .map(|specifier| json!({ "documentation": specifier }))
}

/// Generate a data value for a completion item that will instruct the client to
/// resolve the completion item to obtain further information, in this case, the
/// details/documentation endpoint for the item if it exists in the registry
/// configuration when there is a match result that should be interpolated
fn get_data_with_match(
  registry: &RegistryConfiguration,
  base: &ModuleSpecifier,
  tokens: &[Token],
  match_result: &MatchResult,
  variable: &Key,
  value: &str,
) -> Option<Value> {
  let url = registry.get_documentation_url_for_key(variable)?;
  get_endpoint_with_match(
    variable,
    url,
    base,
    tokens,
    match_result,
    Some(value),
  )
  .ok()
  .map(|specifier| json!({ "documentation": specifier }))
}

/// Convert a single variable templated string into a fully qualified URL which
/// can be fetched to provide additional data.
fn get_endpoint(
  url: &str,
  base: &Url,
  variable: &Key,
  maybe_value: Option<&str>,
) -> Result<ModuleSpecifier, AnyError> {
  let url = replace_variable(url, variable, maybe_value);
  parse_url_with_base(&url, base)
}

/// Convert a templated URL string into a fully qualified URL which can be
/// fetched to provide additional data. If `maybe_value` is some, then the
/// variable will replaced in the template prior to other matched variables
/// being replaced, otherwise the supplied variable will be blanked out if
/// present in the template.
fn get_endpoint_with_match(
  variable: &Key,
  url: &str,
  base: &Url,
  tokens: &[Token],
  match_result: &MatchResult,
  maybe_value: Option<&str>,
) -> Result<ModuleSpecifier, AnyError> {
  let mut url = url.to_string();
  let has_value = maybe_value.is_some();
  if has_value {
    url = replace_variable(&url, variable, maybe_value);
  }
  for (key, value) in match_result.params.iter() {
    if let StringOrNumber::String(name) = key {
      let maybe_key = tokens.iter().find_map(|t| match t {
        Token::Key(k) if k.name == *key => Some(k),
        _ => None,
      });
      url =
        url.replace(&format!("${{{name}}}"), &value.to_string(maybe_key, true));
      url = url.replace(
        &format!("${{{{{name}}}}}"),
        &percent_encoding::percent_encode(
          value.to_string(maybe_key, true).as_bytes(),
          COMPONENT,
        )
        .to_string(),
      );
    }
  }
  if !has_value {
    url = replace_variable(&url, variable, None);
  }
  parse_url_with_base(&url, base)
}

/// Based on the preselect response from the registry, determine if this item
/// should be preselected or not.
fn get_preselect(item: String, preselect: Option<String>) -> Option<bool> {
  if Some(item) == preselect {
    Some(true)
  } else {
    None
  }
}

fn parse_replacement_variables<S: AsRef<str>>(s: S) -> Vec<String> {
  REPLACEMENT_VARIABLE_RE
    .captures_iter(s.as_ref())
    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
    .collect()
}

/// Attempt to parse a URL along with a base, where the base will be used if the
/// URL requires one.
fn parse_url_with_base(
  url: &str,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  match Url::parse(url) {
    Ok(url) => Ok(url),
    Err(ParseError::RelativeUrlWithoutBase) => {
      base.join(url).map_err(|err| err.into())
    }
    Err(err) => Err(err.into()),
  }
}

/// Replaces a variable in a templated URL string with the supplied value or
/// "blank" it out if there is no value supplied.
fn replace_variable(
  url: &str,
  variable: &Key,
  maybe_value: Option<&str>,
) -> String {
  let url_str = url.to_string();
  let value = maybe_value.unwrap_or("");
  if let StringOrNumber::String(name) = &variable.name {
    url_str
      .replace(&format!("${{{name}}}"), value)
      .replace(&format! {"${{{{{name}}}}}"}, value)
  } else {
    url_str
  }
}

/// Validate a registry configuration JSON structure.
fn validate_config(config: &RegistryConfigurationJson) -> Result<(), AnyError> {
  if config.version < 1 || config.version > 2 {
    return Err(anyhow!(
      "Invalid registry configuration. Expected version 1 or 2 got {}.",
      config.version
    ));
  }
  for registry in &config.registries {
    let (_, keys) = string_to_regex(&registry.schema, None)?;
    let key_names: Vec<String> = keys
      .map(|keys| {
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
      })
      .unwrap_or_default();

    for key_name in &key_names {
      if !registry
        .variables
        .iter()
        .map(|var| var.key.to_owned())
        .any(|x| x == *key_name)
      {
        return Err(anyhow!("Invalid registry configuration. Registry with schema \"{}\" is missing variable declaration for key \"{}\".", registry.schema, key_name));
      }
    }

    for variable in &registry.variables {
      let key_index = key_names.iter().position(|key| *key == variable.key);
      let key_index = key_index.ok_or_else(||anyhow!("Invalid registry configuration. Registry with schema \"{}\" is missing a path parameter in schema for variable \"{}\".", registry.schema, variable.key))?;

      let replacement_variables = parse_replacement_variables(&variable.url);
      let limited_keys = key_names.get(0..key_index).unwrap();
      for v in replacement_variables {
        if variable.key == v && config.version == 1 {
          return Err(anyhow!("Invalid registry configuration. Url \"{}\" (for variable \"{}\" in registry with schema \"{}\") uses variable \"{}\", which is not allowed because that would be a self reference.", variable.url, variable.key, registry.schema, v));
        }

        let key_index = limited_keys.iter().position(|key| key == &v);

        if key_index.is_none() && variable.key != v {
          return Err(anyhow!("Invalid registry configuration. Url \"{}\" (for variable \"{}\" in registry with schema \"{}\") uses variable \"{}\", which is not allowed because the schema defines \"{}\" to the right of \"{}\".", variable.url, variable.key, registry.schema, v, v, variable.key));
        }
      }
    }
  }

  Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryConfigurationVariable {
  /// The name of the variable.
  key: String,
  /// An optional URL/API endpoint that can provide optional documentation for a
  /// completion item when requested by the language server.
  documentation: Option<String>,
  /// The URL with variable substitutions of the endpoint that will provide
  /// completions for the variable.
  url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryConfiguration {
  /// A Express-like path which describes how URLs are composed for a registry.
  schema: String,
  /// The variables denoted in the `schema` should have a variable entry.
  variables: Vec<RegistryConfigurationVariable>,
}

impl RegistryConfiguration {
  fn get_url_for_key(&self, key: &Key) -> Option<&str> {
    self.variables.iter().find_map(|v| {
      if key.name == StringOrNumber::String(v.key.clone()) {
        Some(v.url.as_str())
      } else {
        None
      }
    })
  }

  fn get_documentation_url_for_key(&self, key: &Key) -> Option<&str> {
    self.variables.iter().find_map(|v| {
      if key.name == StringOrNumber::String(v.key.clone()) {
        v.documentation.as_deref()
      } else {
        None
      }
    })
  }
}

/// A structure that represents the configuration of an origin and its module
/// registries.
#[derive(Debug, Deserialize)]
struct RegistryConfigurationJson {
  version: u32,
  registries: Vec<RegistryConfiguration>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VariableItemsList {
  pub items: Vec<String>,
  #[serde(default)]
  pub is_incomplete: bool,
  pub preselect: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum VariableItems {
  Simple(Vec<String>),
  List(VariableItemsList),
}

/// A structure which holds the information about currently configured module
/// registries and can provide completion information for URLs that match
/// one of the enabled registries.
#[derive(Debug, Clone)]
pub struct ModuleRegistry {
  origins: HashMap<String, Vec<RegistryConfiguration>>,
  pub location: PathBuf,
  pub file_fetcher: Arc<FileFetcher>,
  http_cache: Arc<GlobalHttpCache>,
}

impl ModuleRegistry {
  pub fn new(
    location: PathBuf,
    http_client_provider: Arc<HttpClientProvider>,
  ) -> Self {
    // the http cache should always be the global one for registry completions
    let http_cache = Arc::new(GlobalHttpCache::new(
      location.clone(),
      crate::cache::RealDenoCacheEnv,
    ));
    let mut file_fetcher = FileFetcher::new(
      http_cache.clone(),
      CacheSetting::RespectHeaders,
      true,
      http_client_provider,
      Default::default(),
      None,
    );
    file_fetcher.set_download_log_level(super::logging::lsp_log_level());

    Self {
      origins: HashMap::new(),
      location,
      file_fetcher: Arc::new(file_fetcher),
      http_cache,
    }
  }

  /// Disable a registry, removing its configuration, if any, from memory.
  pub fn disable(&mut self, origin: &str) {
    let Ok(origin_url) = Url::parse(origin) else {
      return;
    };
    let origin = base_url(&origin_url);
    self.origins.remove(&origin);
  }

  /// Check to see if the given origin has a registry configuration.
  pub async fn check_origin(&self, origin: &str) -> Result<(), AnyError> {
    let origin_url = Url::parse(origin)?;
    let specifier = origin_url.join(CONFIG_PATH)?;
    self.fetch_config(&specifier).await?;
    Ok(())
  }

  /// Fetch and validate the specifier to a registry configuration, resolving
  /// with the configuration if valid.
  async fn fetch_config(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Vec<RegistryConfiguration>, AnyError> {
    // spawn due to the lsp's `Send` requirement
    let fetch_result = deno_core::unsync::spawn({
      let file_fetcher = self.file_fetcher.clone();
      let specifier = specifier.clone();
      async move {
        file_fetcher
        .fetch_with_options(FetchOptions {
          specifier: &specifier,
          permissions: FetchPermissionsOptionRef::AllowAll,
          maybe_accept: Some("application/vnd.deno.reg.v2+json, application/vnd.deno.reg.v1+json;q=0.9, application/json;q=0.8"),
          maybe_cache_setting: None,
        })
        .await
      }
    }).await?;
    // if there is an error fetching, we will cache an empty file, so that
    // subsequent requests they are just an empty doc which will error without
    // needing to connect to the remote URL. We will cache it for 1 week.
    if fetch_result.is_err() {
      let mut headers_map = HashMap::new();
      headers_map.insert(
        "cache-control".to_string(),
        "max-age=604800, immutable".to_string(),
      );
      self.http_cache.set(specifier, headers_map, &[])?;
    }
    let file = fetch_result?.into_text_decoded()?;
    let config: RegistryConfigurationJson = serde_json::from_str(&file.source)?;
    validate_config(&config)?;
    Ok(config.registries)
  }

  /// Enable a registry by attempting to retrieve its configuration and
  /// validating it.
  pub async fn enable(&mut self, origin: &str) {
    let Ok(origin_url) = Url::parse(origin) else {
      return;
    };
    let origin = base_url(&origin_url);
    #[allow(clippy::map_entry)]
    // we can't use entry().or_insert_with() because we can't use async closures
    if !self.origins.contains_key(&origin) {
      let Ok(specifier) = origin_url.join(CONFIG_PATH) else {
        return;
      };
      match self.fetch_config(&specifier).await {
        Ok(configs) => {
          self.origins.insert(origin, configs);
        }
        Err(err) => {
          lsp_log!(
            "  Error fetching registry config for \"{}\": {}",
            origin,
            err.to_string()
          );
          self.origins.remove(&origin);
        }
      }
    }
  }

  #[cfg(test)]
  /// This is only used during testing, as it directly provides the full URL
  /// for obtaining the registry configuration, versus "guessing" at it.
  async fn enable_custom(&mut self, specifier: &str) -> Result<(), AnyError> {
    let specifier = Url::parse(specifier)?;
    let origin = base_url(&specifier);
    #[allow(clippy::map_entry)]
    if !self.origins.contains_key(&origin) {
      let configs = self.fetch_config(&specifier).await?;
      self.origins.insert(origin, configs);
    }

    Ok(())
  }

  pub async fn get_hover(&self, dependency: &Dependency) -> Option<String> {
    let maybe_code = dependency.get_code();
    let maybe_type = dependency.get_type();
    let specifier = match (maybe_code, maybe_type) {
      (Some(specifier), _) => Some(specifier),
      (_, Some(specifier)) => Some(specifier),
      _ => None,
    }?;
    let origin = base_url(specifier);
    let registries = self.origins.get(&origin)?;
    let path = &specifier[Position::BeforePath..];
    for registry in registries {
      let tokens = parse(&registry.schema, None).ok()?;
      let matcher = Matcher::new(&tokens, None).ok()?;
      if let Some(match_result) = matcher.matches(path) {
        let key = if let Some(Token::Key(key)) = tokens.iter().last() {
          Some(key)
        } else {
          None
        }?;
        let url = registry.get_documentation_url_for_key(key)?;
        let endpoint = get_endpoint_with_match(
          key,
          url,
          specifier,
          &tokens,
          &match_result,
          None,
        )
        .ok()?;
        let file_fetcher = self.file_fetcher.clone();
        // spawn due to the lsp's `Send` requirement
        let file = deno_core::unsync::spawn({
          async move {
            file_fetcher
              .fetch_bypass_permissions(&endpoint)
              .await
              .ok()?
              .into_text_decoded()
              .ok()
          }
        })
        .await
        .ok()??;
        let documentation: lsp::Documentation =
          serde_json::from_str(&file.source).ok()?;
        return match documentation {
          lsp::Documentation::String(doc) => Some(doc),
          lsp::Documentation::MarkupContent(lsp::MarkupContent {
            value,
            ..
          }) => Some(value),
        };
      }
    }

    None
  }

  /// For a string specifier from the client, provide a set of completions, if
  /// any, for the specifier.
  pub async fn get_completions(
    &self,
    text: &str,
    range: &lsp::Range,
    resolved: Option<&ModuleSpecifier>,
    specifier_exists: impl Fn(&ModuleSpecifier) -> bool,
  ) -> Option<lsp::CompletionList> {
    let resolved = resolved
      .map(Cow::Borrowed)
      .or_else(|| ModuleSpecifier::parse(text).ok().map(Cow::Owned))?;
    let resolved_str = resolved.as_str();
    let origin = base_url(&resolved);
    let origin_char_count = origin.chars().count();
    let registries = self.origins.get(&origin)?;
    let path = &resolved[Position::BeforePath..];
    let path_char_offset = resolved_str.chars().count() - origin_char_count;
    let mut completions = HashMap::<String, lsp::CompletionItem>::new();
    let mut is_incomplete = false;
    let mut did_match = false;
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
      let last_key_name = StringOrNumber::String(
        tokens
          .iter()
          .last()
          .map(|t| {
            if let Token::Key(key) = t {
              if let StringOrNumber::String(s) = &key.name {
                return s.clone();
              }
            }
            "".to_string()
          })
          .unwrap_or_default(),
      );
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
          did_match = true;
          let completion_type =
            get_completion_type(path_char_offset, &tokens, &match_result);
          match completion_type {
            Some(CompletionType::Literal(s)) => {
              let label = s;
              let full_text = format!("{text}{label}");
              let text_edit =
                Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                  range: *range,
                  new_text: full_text.clone(),
                }));
              let filter_text = Some(full_text);
              completions.insert(
                label.clone(),
                lsp::CompletionItem {
                  label,
                  kind: Some(lsp::CompletionItemKind::FOLDER),
                  filter_text,
                  sort_text: Some("1".to_string()),
                  text_edit,
                  commit_characters: Some(
                    REGISTRY_IMPORT_COMMIT_CHARS
                      .iter()
                      .map(|&c| c.into())
                      .collect(),
                  ),
                  ..Default::default()
                },
              );
            }
            Some(CompletionType::Key { key, prefix, index }) => {
              let maybe_url = registry.get_url_for_key(&key);
              if let Some(url) = maybe_url {
                if let Some(items) = self
                  .get_variable_items(
                    &key,
                    url,
                    &resolved,
                    &tokens,
                    &match_result,
                  )
                  .await
                {
                  let compiler = Compiler::new(&tokens[..=index], None);
                  let base = Url::parse(&origin).ok()?;
                  let (items, preselect, incomplete) = match items {
                    VariableItems::List(list) => {
                      (list.items, list.preselect, list.is_incomplete)
                    }
                    VariableItems::Simple(items) => (items, None, false),
                  };
                  if incomplete {
                    is_incomplete = true;
                  }
                  for (idx, item) in items.into_iter().enumerate() {
                    let mut label = if let Some(p) = &prefix {
                      format!("{p}{item}")
                    } else {
                      item.clone()
                    };
                    if label.ends_with('/') {
                      label.pop();
                    }
                    let kind =
                      if key.name == last_key_name && !item.ends_with('/') {
                        Some(lsp::CompletionItemKind::FILE)
                      } else {
                        Some(lsp::CompletionItemKind::FOLDER)
                      };
                    let mut params = match_result.params.clone();
                    params.insert(
                      key.name.clone(),
                      StringOrVec::from_str(&item, &key),
                    );
                    let mut path =
                      compiler.to_path(&params).unwrap_or_default();
                    if path.ends_with('/') {
                      path.pop();
                    }
                    let item_specifier = base.join(&path).ok()?;
                    let full_text = if let Some(suffix) =
                      item_specifier.as_str().strip_prefix(resolved_str)
                    {
                      format!("{text}{suffix}")
                    } else {
                      item_specifier.to_string()
                    };
                    let text_edit =
                      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                        range: *range,
                        new_text: full_text.to_string(),
                      }));
                    let command = if key.name == last_key_name
                      && !item.ends_with('/')
                      && !specifier_exists(&item_specifier)
                    {
                      Some(lsp::Command {
                        title: "".to_string(),
                        command: "deno.cache".to_string(),
                        arguments: Some(vec![
                          json!([item_specifier]),
                          json!(&resolved),
                        ]),
                      })
                    } else {
                      None
                    };
                    let detail = Some(format!("({})", key.name));
                    let filter_text = Some(full_text.to_string());
                    let sort_text = Some(format!("{:0>10}", idx + 1));
                    let preselect =
                      get_preselect(item.clone(), preselect.clone());
                    let data = get_data_with_match(
                      registry,
                      &resolved,
                      &tokens,
                      &match_result,
                      &key,
                      &item,
                    );
                    let commit_characters = if is_incomplete {
                      Some(
                        REGISTRY_IMPORT_COMMIT_CHARS
                          .iter()
                          .map(|&c| c.into())
                          .collect(),
                      )
                    } else {
                      Some(
                        IMPORT_COMMIT_CHARS.iter().map(|&c| c.into()).collect(),
                      )
                    };
                    completions.insert(
                      item,
                      lsp::CompletionItem {
                        label,
                        kind,
                        detail,
                        sort_text,
                        filter_text,
                        text_edit,
                        command,
                        preselect,
                        data,
                        commit_characters,
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
        // If we have fallen though to the first token, and we still
        // didn't get a match
        if i == 0 {
          match &tokens[i] {
            // so if the first token is a string literal, we will return
            // that as a suggestion
            Token::String(s) => {
              if s.starts_with(path) {
                let label = s.to_string();
                let kind = Some(lsp::CompletionItemKind::FOLDER);
                let mut url = resolved.as_ref().clone();
                url.set_path(s);
                let full_text = if let Some(suffix) =
                  url.as_str().strip_prefix(resolved_str)
                {
                  format!("{text}{suffix}")
                } else {
                  url.to_string()
                };
                let text_edit =
                  Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                    range: *range,
                    new_text: full_text.to_string(),
                  }));
                let filter_text = Some(full_text.to_string());
                completions.insert(
                  s.to_string(),
                  lsp::CompletionItem {
                    label,
                    kind,
                    filter_text,
                    sort_text: Some("1".to_string()),
                    text_edit,
                    preselect: Some(true),
                    commit_characters: Some(
                      REGISTRY_IMPORT_COMMIT_CHARS
                        .iter()
                        .map(|&c| c.into())
                        .collect(),
                    ),
                    ..Default::default()
                  },
                );
              }
            }
            // if the token though is a key, and the key has a prefix, and
            // the path matches the prefix, we will go and get the items
            // for that first key and return them.
            Token::Key(k) => {
              if let Some(prefix) = &k.prefix {
                let maybe_url = registry.get_url_for_key(k);
                if let Some(url) = maybe_url {
                  if let Some(items) = self.get_items(url).await {
                    let base = Url::parse(&origin).ok()?;
                    let (items, preselect, incomplete) = match items {
                      VariableItems::List(list) => {
                        (list.items, list.preselect, list.is_incomplete)
                      }
                      VariableItems::Simple(items) => (items, None, false),
                    };
                    if incomplete {
                      is_incomplete = true;
                    }
                    for (idx, item) in items.into_iter().enumerate() {
                      let path = format!("{prefix}{item}");
                      let kind = Some(lsp::CompletionItemKind::FOLDER);
                      let item_specifier = base.join(&path).ok()?;
                      let full_text = if let Some(suffix) =
                        item_specifier.as_str().strip_prefix(resolved_str)
                      {
                        format!("{text}{suffix}")
                      } else {
                        item_specifier.to_string()
                      };
                      let text_edit =
                        Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                          range: *range,
                          new_text: full_text.clone(),
                        }));
                      let command = if k.name == last_key_name
                        && !specifier_exists(&item_specifier)
                      {
                        Some(lsp::Command {
                          title: "".to_string(),
                          command: "deno.cache".to_string(),
                          arguments: Some(vec![
                            json!([item_specifier]),
                            json!(&resolved),
                          ]),
                        })
                      } else {
                        None
                      };
                      let detail = Some(format!("({})", k.name));
                      let filter_text = Some(full_text.to_string());
                      let sort_text = Some(format!("{:0>10}", idx + 1));
                      let preselect =
                        get_preselect(item.clone(), preselect.clone());
                      let data = get_data(registry, &resolved, k, &path);
                      let commit_characters = if is_incomplete {
                        Some(
                          REGISTRY_IMPORT_COMMIT_CHARS
                            .iter()
                            .map(|&c| c.into())
                            .collect(),
                        )
                      } else {
                        Some(
                          IMPORT_COMMIT_CHARS
                            .iter()
                            .map(|&c| c.into())
                            .collect(),
                        )
                      };
                      completions.insert(
                        item.clone(),
                        lsp::CompletionItem {
                          label: item,
                          kind,
                          detail,
                          sort_text,
                          filter_text,
                          text_edit,
                          command,
                          preselect,
                          data,
                          commit_characters,
                          ..Default::default()
                        },
                      );
                    }
                  }
                }
              }
            }
          }
          break;
        }
      }
    }
    // If we return None, other sources of completions will be looked for
    // but if we did at least match part of a registry, we should send an
    // empty vector so that no-completions will be sent back to the client
    if completions.is_empty() && !did_match {
      None
    } else {
      Some(lsp::CompletionList {
        items: completions.into_values().collect(),
        is_incomplete,
      })
    }
  }

  pub async fn get_documentation(
    &self,
    url: &str,
  ) -> Option<lsp::Documentation> {
    let specifier = Url::parse(url).ok()?;
    let file_fetcher = self.file_fetcher.clone();
    // spawn due to the lsp's `Send` requirement
    let file = deno_core::unsync::spawn(async move {
      file_fetcher
        .fetch_bypass_permissions(&specifier)
        .await
        .ok()?
        .into_text_decoded()
        .ok()
    })
    .await
    .ok()??;
    serde_json::from_str(&file.source).ok()
  }

  pub fn get_origin_completions(
    &self,
    current_specifier: &str,
    range: &lsp::Range,
  ) -> Option<lsp::CompletionList> {
    let items = self
      .origins
      .keys()
      .filter_map(|k| {
        let mut origin = k.to_string();
        if origin.ends_with('/') {
          origin.pop();
        }
        if origin.starts_with(current_specifier) {
          let text_edit = Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
            range: *range,
            new_text: origin.clone(),
          }));
          Some(lsp::CompletionItem {
            label: origin,
            kind: Some(lsp::CompletionItemKind::FOLDER),
            detail: Some("(registry)".to_string()),
            sort_text: Some("2".to_string()),
            text_edit,
            commit_characters: Some(
              REGISTRY_IMPORT_COMMIT_CHARS
                .iter()
                .map(|&c| c.into())
                .collect(),
            ),
            ..Default::default()
          })
        } else {
          None
        }
      })
      .collect::<Vec<lsp::CompletionItem>>();
    if !items.is_empty() {
      Some(lsp::CompletionList {
        items,
        is_incomplete: false,
      })
    } else {
      None
    }
  }

  async fn get_items(&self, url: &str) -> Option<VariableItems> {
    let specifier = ModuleSpecifier::parse(url).ok()?;
    // spawn due to the lsp's `Send` requirement
    let file = deno_core::unsync::spawn({
      let file_fetcher = self.file_fetcher.clone();
      let specifier = specifier.clone();
      async move {
        file_fetcher
          .fetch_bypass_permissions(&specifier)
          .await
          .map_err(|err| {
            error!(
              "Internal error fetching endpoint \"{}\". {}",
              specifier, err
            );
          })
          .ok()?
          .into_text_decoded()
          .ok()
      }
    })
    .await
    .ok()??;
    let items: VariableItems = serde_json::from_str(&file.source)
      .map_err(|err| {
        error!(
          "Error parsing response from endpoint \"{}\". {}",
          specifier, err
        );
      })
      .ok()?;
    Some(items)
  }

  async fn get_variable_items(
    &self,
    variable: &Key,
    url: &str,
    base: &Url,
    tokens: &[Token],
    match_result: &MatchResult,
  ) -> Option<VariableItems> {
    let specifier =
      get_endpoint_with_match(variable, url, base, tokens, match_result, None)
        .map_err(|err| {
          error!("Internal error mapping endpoint \"{}\". {}", url, err);
        })
        .ok()?;
    // spawn due to the lsp's `Send` requirement
    let file = deno_core::unsync::spawn({
      let file_fetcher = self.file_fetcher.clone();
      let specifier = specifier.clone();
      async move {
        file_fetcher
          .fetch_bypass_permissions(&specifier)
          .await
          .map_err(|err| {
            error!(
              "Internal error fetching endpoint \"{}\". {}",
              specifier, err
            );
          })
          .ok()?
          .into_text_decoded()
          .ok()
      }
    })
    .await
    .ok()??;
    let items: VariableItems = serde_json::from_str(&file.source)
      .map_err(|err| {
        error!(
          "Error parsing response from endpoint \"{}\". {}",
          specifier, err
        );
      })
      .ok()?;
    Some(items)
  }

  pub fn clear_cache(&self) {
    self.file_fetcher.clear_memory_files();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use test_util::TempDir;

  #[test]
  fn test_validate_registry_configuration() {
    assert!(validate_config(&RegistryConfigurationJson {
      version: 3,
      registries: vec![],
    })
    .is_err());

    let cfg = RegistryConfigurationJson {
      version: 1,
      registries: vec![RegistryConfiguration {
        schema: "/:module@:version/:path*".to_string(),
        variables: vec![
          RegistryConfigurationVariable {
            key: "module".to_string(),
            documentation: None,
            url: "https://api.deno.land/modules?short".to_string(),
          },
          RegistryConfigurationVariable {
            key: "version".to_string(),
            documentation: None,
            url: "https://deno.land/_vsc1/module/${module}".to_string(),
          },
        ],
      }],
    };
    assert!(validate_config(&cfg).is_err());

    let cfg = RegistryConfigurationJson {
      version: 1,
      registries: vec![RegistryConfiguration {
        schema: "/:module@:version/:path*".to_string(),
        variables: vec![
          RegistryConfigurationVariable {
            key: "module".to_string(),
            documentation: None,
            url: "https://api.deno.land/modules?short".to_string(),
          },
          RegistryConfigurationVariable {
            key: "version".to_string(),
            documentation: None,
            url: "https://deno.land/_vsc1/module/${module}/${path}".to_string(),
          },
          RegistryConfigurationVariable {
            key: "path".to_string(),
            documentation: None,
            url: "https://deno.land/_vsc1/module/${module}/v/${{version}}"
              .to_string(),
          },
        ],
      }],
    };
    assert!(validate_config(&cfg).is_err());

    let cfg = RegistryConfigurationJson {
      version: 1,
      registries: vec![RegistryConfiguration {
        schema: "/:module@:version/:path*".to_string(),
        variables: vec![
          RegistryConfigurationVariable {
            key: "module".to_string(),
            documentation: None,
            url: "https://api.deno.land/modules?short".to_string(),
          },
          RegistryConfigurationVariable {
            key: "version".to_string(),
            documentation: None,
            url: "https://deno.land/_vsc1/module/${module}/v/${{version}}"
              .to_string(),
          },
          RegistryConfigurationVariable {
            key: "path".to_string(),
            documentation: None,
            url: "https://deno.land/_vsc1/module/${module}/v/${{version}}"
              .to_string(),
          },
        ],
      }],
    };
    assert!(validate_config(&cfg).is_err());

    let cfg = RegistryConfigurationJson {
      version: 1,
      registries: vec![RegistryConfiguration {
        schema: "/:module@:version/:path*".to_string(),
        variables: vec![
          RegistryConfigurationVariable {
            key: "module".to_string(),
            documentation: None,
            url: "https://api.deno.land/modules?short".to_string(),
          },
          RegistryConfigurationVariable {
            key: "version".to_string(),
            documentation: None,
            url: "https://deno.land/_vsc1/module/${module}".to_string(),
          },
          RegistryConfigurationVariable {
            key: "path".to_string(),
            documentation: None,
            url: "https://deno.land/_vsc1/module/${module}/v/${{version}}"
              .to_string(),
          },
        ],
      }],
    };
    assert!(validate_config(&cfg).is_ok());

    let cfg: RegistryConfigurationJson = serde_json::from_value(json!({
      "version": 2,
      "registries": [
        {
          "schema": "/x/:module([a-z0-9_]+)@:version?/:path",
          "variables": [
            {
              "key": "module",
              "documentation": "/api/details/mods/${module}",
              "url": "/api/mods/${module}"
            },
            {
              "key": "version",
              "documentation": "/api/details/mods/${module}/v/${{version}}",
              "url": "/api/mods/${module}/v/${{version}}"
            },
            {
              "key": "path",
              "documentation": "/api/details/mods/${module}/v/${{version}}/p/${path}",
              "url": "/api/mods/${module}/v/${{version}}/p/${path}"
            }
          ]
        },
        {
          "schema": "/x/:module([a-z0-9_]+)/:path",
          "variables": [
            {
              "key": "module",
              "documentation": "/api/details/mods/${module}",
              "url": "/api/mods/${module}"
            },
            {
              "key": "path",
              "documentation": "/api/details/mods/${module}/v/latest/p/${path}",
              "url": "/api/mods/${module}/v/latest/p/${path}"
            }
          ]
        }
      ]
    })).unwrap();
    assert!(validate_config(&cfg).is_ok());
  }

  #[tokio::test]
  async fn test_registry_completions_origin_match() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("registries").to_path_buf();
    let mut module_registry = ModuleRegistry::new(
      location,
      Arc::new(HttpClientProvider::new(None, None)),
    );
    module_registry.enable("http://localhost:4545/").await;
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
    let completions = module_registry.get_origin_completions("h", &range);
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].label, "http://localhost:4545");
    assert_eq!(
      completions[0].text_edit,
      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
        range,
        new_text: "http://localhost:4545".to_string()
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
    let completions =
      module_registry.get_origin_completions("http://localhost", &range);
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].label, "http://localhost:4545");
    assert_eq!(
      completions[0].text_edit,
      Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
        range,
        new_text: "http://localhost:4545".to_string()
      }))
    );
  }

  #[tokio::test]
  async fn test_registry_completions() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("registries").to_path_buf();
    let mut module_registry = ModuleRegistry::new(
      location,
      Arc::new(HttpClientProvider::new(None, None)),
    );
    module_registry.enable("http://localhost:4545/").await;
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 41,
      },
    };
    let completions = module_registry
      .get_completions("http://localhost:4545", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 3);
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
    let completions = module_registry
      .get_completions("http://localhost:4545/", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 3);
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
      .get_completions("http://localhost:4545/x/", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.items.len(), 2);
    assert!(completions.is_incomplete);
    assert!(
      completions.items[0].label == *"a" || completions.items[0].label == *"b"
    );
    assert!(
      completions.items[1].label == *"a" || completions.items[1].label == *"b"
    );

    // testing for incremental searching for a module
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 45,
      },
    };
    let completions = module_registry
      .get_completions("http://localhost:4545/x/a", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap();
    assert_eq!(completions.items.len(), 4);
    assert!(!completions.is_incomplete);
    assert_eq!(
      completions.items[0].data,
      Some(json!({
        "documentation": format!("http://localhost:4545/lsp/registries/doc_{}.json", completions.items[0].label),
      }))
    );

    // testing getting the documentation
    let documentation = module_registry
      .get_documentation("http://localhost:4545/lsp/registries/doc_a.json")
      .await;
    assert_eq!(
      documentation,
      Some(lsp::Documentation::MarkupContent(lsp::MarkupContent {
        kind: lsp::MarkupKind::Markdown,
        value: "**a**".to_string(),
      }))
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
      .get_completions("http://localhost:4545/x/a@", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 3);
    assert_eq!(
      completions[0].data,
      Some(json!({
        "documentation": format!("http://localhost:4545/lsp/registries/doc_a_{}.json", completions[0].label),
      }))
    );

    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 49,
      },
    };
    let completions = module_registry
      .get_completions("http://localhost:4545/x/a@v1.", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 2);
    assert_eq!(
      completions[0].data,
      Some(json!({
        "documentation": format!("http://localhost:4545/lsp/registries/doc_a_{}.json", completions[0].label),
      }))
    );

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
        &range,
        None,
        |_| false,
      )
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].detail, Some("(path)".to_string()));
    assert_eq!(completions[0].kind, Some(lsp::CompletionItemKind::FILE));
    assert!(completions[0].command.is_some());
    assert_eq!(completions[1].detail, Some("(path)".to_string()));
    assert_eq!(completions[0].kind, Some(lsp::CompletionItemKind::FILE));
    assert!(completions[1].command.is_some());

    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 54,
      },
    };
    let completions = module_registry
      .get_completions(
        "http://localhost:4545/x/a@v1.0.0/b",
        &range,
        None,
        |_| false,
      )
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].detail, Some("(path)".to_string()));
    assert_eq!(completions[0].kind, Some(lsp::CompletionItemKind::FILE));
    assert!(completions[0].command.is_some());

    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 55,
      },
    };
    let completions = module_registry
      .get_completions(
        "http://localhost:4545/x/a@v1.0.0/b/",
        &range,
        None,
        |_| false,
      )
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].detail, Some("(path)".to_string()));
    assert_eq!(completions[0].kind, Some(lsp::CompletionItemKind::FILE));
    assert!(completions[0].command.is_some());
  }

  #[tokio::test]
  async fn test_registry_completions_key_first() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("registries").to_path_buf();
    let mut module_registry = ModuleRegistry::new(
      location,
      Arc::new(HttpClientProvider::new(None, None)),
    );
    module_registry
      .enable_custom("http://localhost:4545/lsp/registries/deno-import-intellisense-key-first.json")
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
    let completions = module_registry
      .get_completions("http://localhost:4545/", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 3);
    for completion in completions {
      assert!(completion.text_edit.is_some());
      if let lsp::CompletionTextEdit::Edit(edit) = completion.text_edit.unwrap()
      {
        assert_eq!(
          edit.new_text,
          format!("http://localhost:4545/{}", completion.label)
        );
      } else {
        unreachable!("unexpected text edit");
      }
    }

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
      .get_completions("http://localhost:4545/cde@", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    for completion in completions {
      if let Some(filter_text) = completion.filter_text {
        if !"http://localhost:4545/cde@".contains(&filter_text) {
          continue;
        }
      }
      assert!(completion.text_edit.is_some());
      if let lsp::CompletionTextEdit::Edit(edit) = completion.text_edit.unwrap()
      {
        assert_eq!(
          edit.new_text,
          format!("http://localhost:4545/cde@{}", completion.label)
        );
      } else {
        unreachable!("unexpected text edit");
      }
    }
  }

  #[tokio::test]
  async fn test_registry_completions_complex() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("registries").to_path_buf();
    let mut module_registry = ModuleRegistry::new(
      location,
      Arc::new(HttpClientProvider::new(None, None)),
    );
    module_registry
      .enable_custom("http://localhost:4545/lsp/registries/deno-import-intellisense-complex.json")
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
    let completions = module_registry
      .get_completions("http://localhost:4545/", &range, None, |_| false)
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 3);
    for completion in completions {
      assert!(completion.text_edit.is_some());
      if let lsp::CompletionTextEdit::Edit(edit) = completion.text_edit.unwrap()
      {
        assert_eq!(
          edit.new_text,
          format!("http://localhost:4545/{}", completion.label)
        );
      } else {
        unreachable!("unexpected text edit");
      }
    }
  }

  #[tokio::test]
  async fn test_registry_completions_import_map() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("registries").to_path_buf();
    let mut module_registry = ModuleRegistry::new(
      location,
      Arc::new(HttpClientProvider::new(None, None)),
    );
    module_registry.enable("http://localhost:4545/").await;
    let range = lsp::Range {
      start: lsp::Position {
        line: 0,
        character: 20,
      },
      end: lsp::Position {
        line: 0,
        character: 33,
      },
    };
    let completions = module_registry
      .get_completions(
        "localhost4545/",
        &range,
        Some(&ModuleSpecifier::parse("http://localhost:4545/").unwrap()),
        |_| false,
      )
      .await;
    assert!(completions.is_some());
    let completions = completions.unwrap().items;
    assert_eq!(completions.len(), 3);
    for completion in completions {
      assert!(completion.text_edit.is_some());
      if let lsp::CompletionTextEdit::Edit(edit) = completion.text_edit.unwrap()
      {
        assert_eq!(edit.new_text, format!("localhost4545{}", completion.label));
      } else {
        unreachable!("unexpected text edit");
      }
    }
  }

  #[test]
  fn test_parse_replacement_variables() {
    let actual = parse_replacement_variables(
      "https://deno.land/_vsc1/modules/${module}/v/${{version}}",
    );
    assert_eq!(actual.len(), 2);
    assert!(actual.contains(&"module".to_owned()));
    assert!(actual.contains(&"version".to_owned()));
  }

  #[tokio::test]
  async fn test_check_origin_supported() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("registries").to_path_buf();
    let module_registry = ModuleRegistry::new(
      location,
      Arc::new(HttpClientProvider::new(None, None)),
    );
    let result = module_registry.check_origin("http://localhost:4545").await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_check_origin_not_supported() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("registries").to_path_buf();
    let module_registry = ModuleRegistry::new(
      location,
      Arc::new(HttpClientProvider::new(None, None)),
    );
    let result = module_registry.check_origin("https://example.com").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains(
      "https://example.com/.well-known/deno-import-intellisense.json"
    ));

    // because we are caching an empty file when we hit an error with import
    // detection when fetching the config file, we should have an error now that
    // indicates trying to parse an empty file.
    let result = module_registry.check_origin("https://example.com").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("EOF while parsing a value at line 1 column 0"));
  }
}
