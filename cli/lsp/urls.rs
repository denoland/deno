// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::file_fetcher::map_content_type;

use data_url::DataUrl;
use deno_ast::MediaType;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

/// Used in situations where a default URL needs to be used where otherwise a
/// panic is undesired.
pub static INVALID_SPECIFIER: Lazy<ModuleSpecifier> =
  Lazy::new(|| ModuleSpecifier::parse("deno://invalid").unwrap());

/// Matches the `encodeURIComponent()` encoding from JavaScript, which matches
/// the component percent encoding set.
///
/// See: <https://url.spec.whatwg.org/#component-percent-encode-set>
///
// TODO(@kitsonk) - refactor when #9934 is landed.
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

fn hash_data_specifier(specifier: &ModuleSpecifier) -> String {
  let mut file_name_str = specifier.path().to_string();
  if let Some(query) = specifier.query() {
    file_name_str.push('?');
    file_name_str.push_str(query);
  }
  crate::util::checksum::gen(&[file_name_str.as_bytes()])
}

/// This exists to make it a little bit harder to accidentally use a `Url`
/// in the wrong place where a client url should be used.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct LspClientUrl(Url);

impl LspClientUrl {
  pub fn new(url: Url) -> Self {
    Self(url)
  }

  pub fn as_url(&self) -> &Url {
    &self.0
  }

  pub fn into_url(self) -> Url {
    self.0
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }
}

impl std::fmt::Display for LspClientUrl {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

#[derive(Debug, Default)]
struct LspUrlMapInner {
  specifier_to_url: HashMap<ModuleSpecifier, LspClientUrl>,
  url_to_specifier: HashMap<Url, ModuleSpecifier>,
}

impl LspUrlMapInner {
  fn put(&mut self, specifier: ModuleSpecifier, url: LspClientUrl) {
    self
      .url_to_specifier
      .insert(url.as_url().clone(), specifier.clone());
    self.specifier_to_url.insert(specifier, url);
  }

  fn get_url(&self, specifier: &ModuleSpecifier) -> Option<&LspClientUrl> {
    self.specifier_to_url.get(specifier)
  }

  fn get_specifier(&self, url: &Url) -> Option<&ModuleSpecifier> {
    self.url_to_specifier.get(url)
  }
}

#[derive(Debug, Clone, Copy)]
pub enum LspUrlKind {
  File,
  Folder,
}

/// A bi-directional map of URLs sent to the LSP client and internal module
/// specifiers. We need to map internal specifiers into `deno:` schema URLs
/// to allow the Deno language server to manage these as virtual documents.
#[derive(Debug, Default, Clone)]
pub struct LspUrlMap(Arc<Mutex<LspUrlMapInner>>);

impl LspUrlMap {
  /// Normalize a specifier that is used internally within Deno (or tsc) to a
  /// URL that can be handled as a "virtual" document by an LSP client.
  pub fn normalize_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<LspClientUrl, AnyError> {
    let mut inner = self.0.lock();
    if let Some(url) = inner.get_url(specifier).cloned() {
      Ok(url)
    } else {
      let url = if specifier.scheme() == "file" {
        LspClientUrl(specifier.clone())
      } else {
        let specifier_str = if specifier.scheme() == "asset" {
          format!("deno:/asset{}", specifier.path())
        } else if specifier.scheme() == "data" {
          let data_url = DataUrl::process(specifier.as_str())
            .map_err(|e| uri_error(format!("{e:?}")))?;
          let mime = data_url.mime_type();
          let (media_type, _) =
            map_content_type(specifier, Some(&format!("{mime}")));
          let extension = if media_type == MediaType::Unknown {
            ""
          } else {
            media_type.as_ts_extension()
          };
          format!(
            "deno:/{}/data_url{}",
            hash_data_specifier(specifier),
            extension
          )
        } else {
          let mut path =
            specifier[..Position::BeforePath].replacen("://", "/", 1);
          let parts: Vec<String> = specifier[Position::BeforePath..]
            .split('/')
            .map(|p| {
              percent_encoding::utf8_percent_encode(p, COMPONENT).to_string()
            })
            .collect();
          path.push_str(&parts.join("/"));
          format!("deno:/{path}")
        };
        let url = LspClientUrl(Url::parse(&specifier_str)?);
        inner.put(specifier.clone(), url.clone());
        url
      };
      Ok(url)
    }
  }

  /// Normalize URLs from the client, where "virtual" `deno:///` URLs are
  /// converted into proper module specifiers, as well as handle situations
  /// where the client encodes a file URL differently than Rust does by default
  /// causing issues with string matching of URLs.
  ///
  /// Note: Sometimes the url provided by the client may not have a trailing slash,
  /// so we need to force it to in the mapping and nee to explicitly state whether
  /// this is a file or directory url.
  pub fn normalize_url(&self, url: &Url, kind: LspUrlKind) -> ModuleSpecifier {
    let mut inner = self.0.lock();
    if let Some(specifier) = inner.get_specifier(url).cloned() {
      specifier
    } else {
      let specifier = if let Ok(path) = url.to_file_path() {
        match kind {
          LspUrlKind::Folder => Url::from_directory_path(path).unwrap(),
          LspUrlKind::File => Url::from_file_path(path).unwrap(),
        }
      } else {
        url.clone()
      };
      inner.put(specifier.clone(), LspClientUrl(url.clone()));
      specifier
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;

  #[test]
  fn test_hash_data_specifier() {
    let fixture = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    let actual = hash_data_specifier(&fixture);
    assert_eq!(
      actual,
      "c21c7fc382b2b0553dc0864aa81a3acacfb7b3d1285ab5ae76da6abec213fb37"
    );
  }

  #[test]
  fn test_lsp_url_map() {
    let map = LspUrlMap::default();
    let fixture = resolve_url("https://deno.land/x/pkg@1.0.0/mod.ts").unwrap();
    let actual_url = map
      .normalize_specifier(&fixture)
      .expect("could not handle specifier");
    let expected_url =
      Url::parse("deno:/https/deno.land/x/pkg%401.0.0/mod.ts").unwrap();
    assert_eq!(actual_url.as_url(), &expected_url);

    let actual_specifier =
      map.normalize_url(actual_url.as_url(), LspUrlKind::File);
    assert_eq!(actual_specifier, fixture);
  }

  #[test]
  fn test_lsp_url_map_complex_encoding() {
    // Test fix for #9741 - not properly encoding certain URLs
    let map = LspUrlMap::default();
    let fixture = resolve_url("https://cdn.skypack.dev/-/postcss@v8.2.9-E4SktPp9c0AtxrJHp8iV/dist=es2020,mode=types/lib/postcss.d.ts").unwrap();
    let actual_url = map
      .normalize_specifier(&fixture)
      .expect("could not handle specifier");
    let expected_url = Url::parse("deno:/https/cdn.skypack.dev/-/postcss%40v8.2.9-E4SktPp9c0AtxrJHp8iV/dist%3Des2020%2Cmode%3Dtypes/lib/postcss.d.ts").unwrap();
    assert_eq!(actual_url.as_url(), &expected_url);

    let actual_specifier =
      map.normalize_url(actual_url.as_url(), LspUrlKind::File);
    assert_eq!(actual_specifier, fixture);
  }

  #[test]
  fn test_lsp_url_map_data() {
    let map = LspUrlMap::default();
    let fixture = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    let actual_url = map
      .normalize_specifier(&fixture)
      .expect("could not handle specifier");
    let expected_url = Url::parse("deno:/c21c7fc382b2b0553dc0864aa81a3acacfb7b3d1285ab5ae76da6abec213fb37/data_url.ts").unwrap();
    assert_eq!(actual_url.as_url(), &expected_url);

    let actual_specifier =
      map.normalize_url(actual_url.as_url(), LspUrlKind::File);
    assert_eq!(actual_specifier, fixture);
  }

  #[cfg(windows)]
  #[test]
  fn test_normalize_windows_path() {
    let map = LspUrlMap::default();
    let fixture = resolve_url(
      "file:///c%3A/Users/deno/Desktop/file%20with%20spaces%20in%20name.txt",
    )
    .unwrap();
    let actual = map.normalize_url(&fixture, LspUrlKind::File);
    let expected =
      Url::parse("file:///C:/Users/deno/Desktop/file with spaces in name.txt")
        .unwrap();
    assert_eq!(actual, expected);
  }

  #[cfg(not(windows))]
  #[test]
  fn test_normalize_percent_encoded_path() {
    let map = LspUrlMap::default();
    let fixture = resolve_url(
      "file:///Users/deno/Desktop/file%20with%20spaces%20in%20name.txt",
    )
    .unwrap();
    let actual = map.normalize_url(&fixture, LspUrlKind::File);
    let expected =
      Url::parse("file:///Users/deno/Desktop/file with spaces in name.txt")
        .unwrap();
    assert_eq!(actual, expected);
  }
}
