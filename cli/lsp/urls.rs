// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::file_fetcher::map_content_type;
use crate::media_type::MediaType;

use deno_core::error::AnyError;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;

/// Matches the `encodeURIComponent()` encoding from JavaScript, which matches
/// the component percent encoding set.
///
/// See: https://url.spec.whatwg.org/#component-percent-encode-set
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
  crate::checksum::gen(&[file_name_str.as_bytes()])
}

fn data_url_media_type(specifier: &ModuleSpecifier) -> MediaType {
  let path = specifier.path();
  let mut parts = path.splitn(2, ',');
  let media_type_part =
    percent_encoding::percent_decode_str(parts.next().unwrap())
      .decode_utf8_lossy();
  let (media_type, _) =
    map_content_type(specifier, Some(media_type_part.into()));
  media_type
}

/// A bi-directional map of URLs sent to the LSP client and internal module
/// specifiers.  We need to map internal specifiers into `deno:` schema URLs
/// to allow the Deno language server to manage these as virtual documents.
#[derive(Debug, Default)]
pub struct LspUrlMap {
  specifier_to_url: HashMap<ModuleSpecifier, Url>,
  url_to_specifier: HashMap<Url, ModuleSpecifier>,
}

impl LspUrlMap {
  fn put(&mut self, specifier: ModuleSpecifier, url: Url) {
    self.specifier_to_url.insert(specifier.clone(), url.clone());
    self.url_to_specifier.insert(url, specifier);
  }

  fn get_url(&self, specifier: &ModuleSpecifier) -> Option<&Url> {
    self.specifier_to_url.get(specifier)
  }

  fn get_specifier(&self, url: &Url) -> Option<&ModuleSpecifier> {
    self.url_to_specifier.get(url)
  }

  /// Normalize a specifier that is used internally within Deno (or tsc) to a
  /// URL that can be handled as a "virtual" document by an LSP client.
  pub fn normalize_specifier(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<Url, AnyError> {
    if let Some(url) = self.get_url(specifier) {
      Ok(url.clone())
    } else {
      let url = if specifier.scheme() == "file" {
        specifier.clone()
      } else {
        let specifier_str = if specifier.scheme() == "data" {
          let media_type = data_url_media_type(specifier);
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
          format!("deno:/{}", path)
        };
        let url = Url::parse(&specifier_str)?;
        self.put(specifier.clone(), url.clone());
        url
      };
      Ok(url)
    }
  }

  /// Normalize URLs from the client, where "virtual" `deno:///` URLs are
  /// converted into proper module specifiers, as well as handle situations
  /// where the client encodes a file URL differently than Rust does by default
  /// causing issues with string matching of URLs.
  pub fn normalize_url(&self, url: &Url) -> ModuleSpecifier {
    if let Some(specifier) = self.get_specifier(url) {
      specifier.clone()
    } else if url.scheme() == "file" {
      let path = url.to_file_path().unwrap();
      Url::from_file_path(path).unwrap()
    } else {
      url.clone()
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
  fn test_data_url_media_type() {
    let fixture = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    let actual = data_url_media_type(&fixture);
    assert_eq!(actual, MediaType::TypeScript);

    let fixture = resolve_url("data:application/javascript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    let actual = data_url_media_type(&fixture);
    assert_eq!(actual, MediaType::JavaScript);

    let fixture = resolve_url("data:text/plain;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    let actual = data_url_media_type(&fixture);
    assert_eq!(actual, MediaType::Unknown);
  }

  #[test]
  fn test_lsp_url_map() {
    let mut map = LspUrlMap::default();
    let fixture = resolve_url("https://deno.land/x/pkg@1.0.0/mod.ts").unwrap();
    let actual_url = map
      .normalize_specifier(&fixture)
      .expect("could not handle specifier");
    let expected_url =
      Url::parse("deno:/https/deno.land/x/pkg%401.0.0/mod.ts").unwrap();
    assert_eq!(actual_url, expected_url);

    let actual_specifier = map.normalize_url(&actual_url);
    assert_eq!(actual_specifier, fixture);
  }

  #[test]
  fn test_lsp_url_map_complex_encoding() {
    // Test fix for #9741 - not properly encoding certain URLs
    let mut map = LspUrlMap::default();
    let fixture = resolve_url("https://cdn.skypack.dev/-/postcss@v8.2.9-E4SktPp9c0AtxrJHp8iV/dist=es2020,mode=types/lib/postcss.d.ts").unwrap();
    let actual_url = map
      .normalize_specifier(&fixture)
      .expect("could not handle specifier");
    let expected_url = Url::parse("deno:/https/cdn.skypack.dev/-/postcss%40v8.2.9-E4SktPp9c0AtxrJHp8iV/dist%3Des2020%2Cmode%3Dtypes/lib/postcss.d.ts").unwrap();
    assert_eq!(actual_url, expected_url);

    let actual_specifier = map.normalize_url(&actual_url);
    assert_eq!(actual_specifier, fixture);
  }

  #[test]
  fn test_lsp_url_map_data() {
    let mut map = LspUrlMap::default();
    let fixture = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    let actual_url = map
      .normalize_specifier(&fixture)
      .expect("could not handle specifier");
    let expected_url = Url::parse("deno:/c21c7fc382b2b0553dc0864aa81a3acacfb7b3d1285ab5ae76da6abec213fb37/data_url.ts").unwrap();
    assert_eq!(actual_url, expected_url);

    let actual_specifier = map.normalize_url(&actual_url);
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
    let actual = map.normalize_url(&fixture);
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
    let actual = map.normalize_url(&fixture);
    let expected =
      Url::parse("file:///Users/deno/Desktop/file with spaces in name.txt")
        .unwrap();
    assert_eq!(actual, expected);
  }
}
