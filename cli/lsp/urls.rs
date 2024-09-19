// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::MediaType;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use lsp_types::Uri;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use super::cache::LspCache;
use super::logging::lsp_warn;

/// Used in situations where a default URL needs to be used where otherwise a
/// panic is undesired.
pub static INVALID_SPECIFIER: Lazy<ModuleSpecifier> =
  Lazy::new(|| ModuleSpecifier::parse("deno://invalid").unwrap());

/// Used in situations where a default URL needs to be used where otherwise a
/// panic is undesired.
pub static INVALID_URI: Lazy<Uri> =
  Lazy::new(|| Uri::from_str("deno://invalid").unwrap());

/// Matches the `encodeURIComponent()` encoding from JavaScript, which matches
/// the component percent encoding set.
///
/// See: <https://url.spec.whatwg.org/#component-percent-encode-set>
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
  .add(b'%')
  .add(b'&')
  .add(b'+')
  .add(b',');

/// Characters that may be left unencoded in a `Url` path but not valid in a
/// `Uri` path.
const URL_TO_URI_PATH: &percent_encoding::AsciiSet =
  &percent_encoding::CONTROLS
    .add(b'[')
    .add(b']')
    .add(b'^')
    .add(b'|');

/// Characters that may be left unencoded in a `Url` query but not valid in a
/// `Uri` query.
const URL_TO_URI_QUERY: &percent_encoding::AsciiSet =
  &URL_TO_URI_PATH.add(b'\\').add(b'`').add(b'{').add(b'}');

/// Characters that may be left unencoded in a `Url` fragment but not valid in
/// a `Uri` fragment.
const URL_TO_URI_FRAGMENT: &percent_encoding::AsciiSet =
  &URL_TO_URI_PATH.add(b'#').add(b'\\').add(b'{').add(b'}');

fn hash_data_specifier(specifier: &ModuleSpecifier) -> String {
  let mut file_name_str = specifier.path().to_string();
  if let Some(query) = specifier.query() {
    file_name_str.push('?');
    file_name_str.push_str(query);
  }
  crate::util::checksum::gen(&[file_name_str.as_bytes()])
}

fn to_deno_uri(specifier: &Url) -> String {
  let mut string = String::with_capacity(specifier.as_str().len() + 6);
  string.push_str("deno:/");
  string.push_str(specifier.scheme());
  for p in specifier[Position::BeforeHost..].split('/') {
    string.push('/');
    string.push_str(
      &percent_encoding::utf8_percent_encode(p, COMPONENT).to_string(),
    );
  }
  string
}

fn from_deno_url(url: &Url) -> Option<Url> {
  if url.scheme() != "deno" {
    return None;
  }
  let mut segments = url.path_segments()?;
  let mut string = String::with_capacity(url.as_str().len());
  string.push_str(segments.next()?);
  string.push_str("://");
  string.push_str(
    &percent_encoding::percent_decode(segments.next()?.as_bytes())
      .decode_utf8()
      .ok()?,
  );
  for segment in segments {
    string.push('/');
    string.push_str(
      &percent_encoding::percent_decode(segment.as_bytes())
        .decode_utf8()
        .ok()?,
    );
  }
  Url::parse(&string).ok()
}

#[derive(Debug, Default)]
struct LspUrlMapInner {
  specifier_to_uri: HashMap<ModuleSpecifier, Uri>,
  uri_to_specifier: HashMap<Uri, ModuleSpecifier>,
}

impl LspUrlMapInner {
  fn put(&mut self, specifier: ModuleSpecifier, uri: Uri) {
    self.uri_to_specifier.insert(uri.clone(), specifier.clone());
    self.specifier_to_uri.insert(specifier, uri);
  }

  fn get_uri(&self, specifier: &ModuleSpecifier) -> Option<&Uri> {
    self.specifier_to_uri.get(specifier)
  }

  fn get_specifier(&self, uri: &Uri) -> Option<&ModuleSpecifier> {
    self.uri_to_specifier.get(uri)
  }
}

pub fn uri_parse_unencoded(s: &str) -> Result<Uri, AnyError> {
  url_to_uri(&Url::parse(s)?)
}

pub fn url_to_uri(url: &Url) -> Result<Uri, AnyError> {
  let components = deno_core::url::quirks::internal_components(url);
  let mut input = String::with_capacity(url.as_str().len());
  input.push_str(&url.as_str()[..components.path_start as usize]);
  input.push_str(
    &percent_encoding::utf8_percent_encode(url.path(), URL_TO_URI_PATH)
      .to_string(),
  );
  if let Some(query) = url.query() {
    input.push('?');
    input.push_str(
      &percent_encoding::utf8_percent_encode(query, URL_TO_URI_QUERY)
        .to_string(),
    );
  }
  if let Some(fragment) = url.fragment() {
    input.push('#');
    input.push_str(
      &percent_encoding::utf8_percent_encode(fragment, URL_TO_URI_FRAGMENT)
        .to_string(),
    );
  }
  Ok(Uri::from_str(&input).inspect_err(|err| {
    lsp_warn!("Could not convert URL \"{url}\" to URI: {err}")
  })?)
}

pub fn uri_to_url(uri: &Uri) -> Url {
  Url::parse(uri.as_str()).unwrap()
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
pub struct LspUrlMap {
  cache: LspCache,
  inner: Arc<Mutex<LspUrlMapInner>>,
}

impl LspUrlMap {
  pub fn set_cache(&mut self, cache: &LspCache) {
    self.cache = cache.clone();
  }

  /// Normalize a specifier that is used internally within Deno (or tsc) to a
  /// URL that can be handled as a "virtual" document by an LSP client.
  pub fn specifier_to_uri(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Result<Uri, AnyError> {
    if let Some(file_url) =
      self.cache.vendored_specifier(specifier, file_referrer)
    {
      return url_to_uri(&file_url);
    }
    let mut inner = self.inner.lock();
    if let Some(uri) = inner.get_uri(specifier).cloned() {
      Ok(uri)
    } else {
      let uri = if specifier.scheme() == "file" {
        url_to_uri(specifier)?
      } else {
        let uri_str = if specifier.scheme() == "asset" {
          format!("deno:/asset{}", specifier.path())
        } else if specifier.scheme() == "data" {
          let data_url = deno_graph::source::RawDataUrl::parse(specifier)?;
          let media_type = data_url.media_type();
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
          to_deno_uri(specifier)
        };
        let uri = uri_parse_unencoded(&uri_str)?;
        inner.put(specifier.clone(), uri.clone());
        uri
      };
      Ok(uri)
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
  pub fn uri_to_specifier(
    &self,
    uri: &Uri,
    kind: LspUrlKind,
  ) -> ModuleSpecifier {
    let url = uri_to_url(uri);
    if let Some(remote_url) = self.cache.unvendored_specifier(&url) {
      return remote_url;
    }
    let mut inner = self.inner.lock();
    if let Some(specifier) = inner.get_specifier(uri).cloned() {
      return specifier;
    }
    let mut specifier = None;
    if url.scheme() == "file" {
      if let Ok(path) = url.to_file_path() {
        specifier = Some(match kind {
          LspUrlKind::Folder => Url::from_directory_path(path).unwrap(),
          LspUrlKind::File => Url::from_file_path(path).unwrap(),
        });
      }
    } else if let Some(s) = file_like_to_file_specifier(&url) {
      specifier = Some(s);
    } else if let Some(s) = from_deno_url(&url) {
      specifier = Some(s);
    }
    let specifier = specifier.unwrap_or_else(|| url.clone());
    inner.put(specifier.clone(), uri.clone());
    specifier
  }
}

/// Convert a e.g. `deno-notebook-cell:` specifier to a `file:` specifier.
/// ```rust
/// assert_eq!(
///   file_like_to_file_specifier(
///     &Url::parse("deno-notebook-cell:/path/to/file.ipynb#abc").unwrap(),
///   ),
///   Some(Url::parse("file:///path/to/file.ipynb.ts?scheme=deno-notebook-cell#abc").unwrap()),
/// );
fn file_like_to_file_specifier(specifier: &Url) -> Option<Url> {
  if matches!(specifier.scheme(), "untitled" | "deno-notebook-cell") {
    if let Ok(mut s) = ModuleSpecifier::parse(&format!(
      "file://{}",
      &specifier.as_str()[deno_core::url::quirks::internal_components(specifier)
        .host_end as usize..],
    )) {
      s.query_pairs_mut()
        .append_pair("scheme", specifier.scheme());
      s.set_path(&format!("{}.ts", s.path()));
      return Some(s);
    }
  }
  None
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
    let actual_uri = map
      .specifier_to_uri(&fixture, None)
      .expect("could not handle specifier");
    assert_eq!(
      actual_uri.as_str(),
      "deno:/https/deno.land/x/pkg%401.0.0/mod.ts"
    );
    let actual_specifier = map.uri_to_specifier(&actual_uri, LspUrlKind::File);
    assert_eq!(actual_specifier, fixture);
  }

  #[test]
  fn test_lsp_url_reverse() {
    let map = LspUrlMap::default();
    let fixture =
      Uri::from_str("deno:/https/deno.land/x/pkg%401.0.0/mod.ts").unwrap();
    let actual_specifier = map.uri_to_specifier(&fixture, LspUrlKind::File);
    let expected_specifier =
      Url::parse("https://deno.land/x/pkg@1.0.0/mod.ts").unwrap();
    assert_eq!(&actual_specifier, &expected_specifier);

    let actual_uri = map.specifier_to_uri(&actual_specifier, None).unwrap();
    assert_eq!(actual_uri, fixture);
  }

  #[test]
  fn test_lsp_url_map_complex_encoding() {
    // Test fix for #9741 - not properly encoding certain URLs
    let map = LspUrlMap::default();
    let fixture = resolve_url("https://cdn.skypack.dev/-/postcss@v8.2.9-E4SktPp9c0AtxrJHp8iV/dist=es2020,mode=types/lib/postcss.d.ts").unwrap();
    let actual_uri = map
      .specifier_to_uri(&fixture, None)
      .expect("could not handle specifier");
    assert_eq!(actual_uri.as_str(), "deno:/https/cdn.skypack.dev/-/postcss%40v8.2.9-E4SktPp9c0AtxrJHp8iV/dist%3Des2020%2Cmode%3Dtypes/lib/postcss.d.ts");
    let actual_specifier = map.uri_to_specifier(&actual_uri, LspUrlKind::File);
    assert_eq!(actual_specifier, fixture);
  }

  #[test]
  fn test_lsp_url_map_data() {
    let map = LspUrlMap::default();
    let fixture = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();
    let actual_uri = map
      .specifier_to_uri(&fixture, None)
      .expect("could not handle specifier");
    let expected_url = Url::parse("deno:/c21c7fc382b2b0553dc0864aa81a3acacfb7b3d1285ab5ae76da6abec213fb37/data_url.ts").unwrap();
    assert_eq!(&uri_to_url(&actual_uri), &expected_url);

    let actual_specifier = map.uri_to_specifier(&actual_uri, LspUrlKind::File);
    assert_eq!(actual_specifier, fixture);
  }

  #[test]
  fn test_lsp_url_map_host_with_port() {
    let map = LspUrlMap::default();
    let fixture = resolve_url("http://localhost:8000/mod.ts").unwrap();
    let actual_uri = map
      .specifier_to_uri(&fixture, None)
      .expect("could not handle specifier");
    assert_eq!(actual_uri.as_str(), "deno:/http/localhost%3A8000/mod.ts");
    let actual_specifier = map.uri_to_specifier(&actual_uri, LspUrlKind::File);
    assert_eq!(actual_specifier, fixture);
  }

  #[cfg(windows)]
  #[test]
  fn test_normalize_windows_path() {
    let map = LspUrlMap::default();
    let fixture = Uri::from_str(
      "file:///c%3A/Users/deno/Desktop/file%20with%20spaces%20in%20name.txt",
    )
    .unwrap();
    let actual = map.uri_to_specifier(&fixture, LspUrlKind::File);
    let expected =
      Url::parse("file:///C:/Users/deno/Desktop/file with spaces in name.txt")
        .unwrap();
    assert_eq!(actual, expected);
  }

  #[cfg(not(windows))]
  #[test]
  fn test_normalize_percent_encoded_path() {
    let map = LspUrlMap::default();
    let fixture = Uri::from_str(
      "file:///Users/deno/Desktop/file%20with%20spaces%20in%20name.txt",
    )
    .unwrap();
    let actual = map.uri_to_specifier(&fixture, LspUrlKind::File);
    let expected =
      Url::parse("file:///Users/deno/Desktop/file with spaces in name.txt")
        .unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn test_normalize_deno_status() {
    let map = LspUrlMap::default();
    let fixture = Uri::from_str("deno:/status.md").unwrap();
    let actual = map.uri_to_specifier(&fixture, LspUrlKind::File);
    assert_eq!(actual.as_str(), fixture.as_str());
  }

  #[test]
  fn test_file_like_to_file_specifier() {
    assert_eq!(
      file_like_to_file_specifier(
        &Url::parse("deno-notebook-cell:/path/to/file.ipynb#abc").unwrap(),
      ),
      Some(
        Url::parse(
          "file:///path/to/file.ipynb.ts?scheme=deno-notebook-cell#abc"
        )
        .unwrap()
      ),
    );
    assert_eq!(
      file_like_to_file_specifier(
        &Url::parse("untitled:/path/to/file.ipynb#123").unwrap(),
      ),
      Some(
        Url::parse("file:///path/to/file.ipynb.ts?scheme=untitled#123")
          .unwrap()
      ),
    );
  }
}
