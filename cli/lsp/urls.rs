// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;
use std::str::FromStr;

use deno_config::UrlToFilePathError;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_path_util::url_to_file_path;
use lsp_types::Uri;

use super::logging::lsp_warn;

/// Matches the `encodeURIComponent()` encoding from JavaScript, which matches
/// the component percent encoding set.
///
/// See: <https://url.spec.whatwg.org/#component-percent-encode-set>
pub const COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
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

/// Characters that are left unencoded in a `Url` path but will be encoded in a
/// VSCode URI.
const URL_TO_URI_PATH: &percent_encoding::AsciiSet =
  &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'$')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
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
  (|| {
    let scheme = uri.scheme()?;
    if !scheme.eq_lowercase("untitled")
      && !scheme.eq_lowercase("vscode-notebook-cell")
      && !scheme.eq_lowercase("deno-notebook-cell")
      && !scheme.eq_lowercase("vscode-userdata")
    {
      return None;
    }
    Url::parse(&format!(
      "file:///{}",
      &uri.as_str()[uri.path_bounds.0 as usize..uri.path_bounds.1 as usize]
        .trim_start_matches('/'),
    ))
    .ok()
  })()
  .unwrap_or_else(|| Url::parse(uri.as_str()).unwrap())
}

pub fn uri_to_file_path(uri: &Uri) -> Result<PathBuf, UrlToFilePathError> {
  url_to_file_path(&uri_to_url(uri))
}

pub fn uri_is_file_like(uri: &Uri) -> bool {
  let Some(scheme) = uri.scheme() else {
    return false;
  };
  scheme.eq_lowercase("file")
    || scheme.eq_lowercase("untitled")
    || scheme.eq_lowercase("vscode-notebook-cell")
    || scheme.eq_lowercase("deno-notebook-cell")
    || scheme.eq_lowercase("vscode-userdata")
}
