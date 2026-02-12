// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::path::Prefix;
use std::str::FromStr;

use deno_config::UrlToFilePathError;
use deno_core::error::AnyError;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_path_util::url_to_file_path;
use lsp_types::Uri;

use super::logging::lsp_warn;

pub fn uri_parse_unencoded(s: &str) -> Result<Uri, AnyError> {
  url_to_uri(&Url::parse(s)?)
}

pub fn normalize_uri(uri: &Uri) -> Uri {
  if !uri.scheme().as_str().eq_ignore_ascii_case("file") {
    return uri.normalize().into();
  }
  let Some(path) = uri.to_file_path() else {
    return uri.normalize().into();
  };
  let normalized_path = normalize_path(path);
  let mut encoded_path =
    fluent_uri::pct_enc::EString::<fluent_uri::pct_enc::encoder::Path>::new();
  let mut path_only_has_prefix = false;
  for component in normalized_path.components() {
    match component {
      Component::Prefix(prefix) => {
        path_only_has_prefix = true;
        match prefix.kind() {
          Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
            encoded_path.encode_str::<fluent_uri::pct_enc::encoder::Path>("/");
            let b = [(letter as char).to_ascii_uppercase() as u8];
            encoded_path.encode_str::<fluent_uri::pct_enc::encoder::Path>(
              // SAFETY: Drive letter is ascii.
              unsafe { str::from_utf8_unchecked(&b) },
            );
          }
          Prefix::UNC(..) | Prefix::VerbatimUNC(..) => {
            // These should be carried in `uri.authority()`.
          }
          Prefix::Verbatim(_) | Prefix::DeviceNS(_) => {
            // Not a local path, abort.
            return uri.normalize().into();
          }
        }
      }
      Component::RootDir => {}
      component => {
        path_only_has_prefix = false;
        encoded_path.encode_str::<fluent_uri::pct_enc::encoder::Path>("/");
        encoded_path.encode_str::<fluent_uri::pct_enc::encoder::Path>(
          &component.as_os_str().to_string_lossy(),
        );
      }
    }
  }
  if encoded_path.is_empty() || path_only_has_prefix {
    encoded_path.encode_str::<fluent_uri::pct_enc::encoder::Path>("/");
  }
  fluent_uri::Uri::builder()
    .scheme(fluent_uri::component::Scheme::new_or_panic("file"))
    .optional(fluent_uri::build::Builder::authority, uri.authority())
    .path(encoded_path.as_ref())
    .optional(fluent_uri::build::Builder::query, uri.query())
    .optional(fluent_uri::build::Builder::fragment, uri.fragment())
    .build()
    .expect("component constraints should be met by the above")
    .normalize()
    .into()
}

pub fn url_to_uri(url: &Url) -> Result<Uri, AnyError> {
  let uri_before_path = Uri::from_str(&url[..Position::BeforePath])
    .inspect_err(|err| {
      lsp_warn!("Could not convert URL \"{url}\" to URI: {err}")
    })?;
  let mut encoded_path =
    fluent_uri::pct_enc::EString::<fluent_uri::pct_enc::encoder::Path>::new();
  encoded_path.encode_str::<fluent_uri::pct_enc::encoder::Path>(
    &percent_encoding::percent_decode_str(url.path()).decode_utf8_lossy(),
  );
  let encoded_query = url.query().map(|query| {
    let mut encoded_query = fluent_uri::pct_enc::EString::<
      fluent_uri::pct_enc::encoder::Query,
    >::new();
    encoded_query.encode_str::<fluent_uri::pct_enc::encoder::Query>(query);
    encoded_query
  });
  let encoded_fragment = url.fragment().map(|fragment| {
    let mut encoded_fragment = fluent_uri::pct_enc::EString::<
      fluent_uri::pct_enc::encoder::Fragment,
    >::new();
    encoded_fragment
      .encode_str::<fluent_uri::pct_enc::encoder::Fragment>(fragment);
    encoded_fragment
  });
  let uri = fluent_uri::Uri::builder()
    .scheme(uri_before_path.scheme())
    .optional(
      fluent_uri::build::Builder::authority,
      uri_before_path.authority(),
    )
    .path(encoded_path.as_ref())
    .optional(fluent_uri::build::Builder::query, encoded_query.as_deref())
    .optional(
      fluent_uri::build::Builder::fragment,
      encoded_fragment.as_deref(),
    )
    .build()
    .expect("component constraints should be met by the above")
    .into();
  Ok(normalize_uri(&uri))
}

pub fn uri_to_url(uri: &Uri) -> Url {
  (|| {
    let scheme = uri.scheme();
    if !scheme.as_str().eq_ignore_ascii_case("untitled")
      && !scheme.as_str().eq_ignore_ascii_case("vscode-notebook-cell")
      && !scheme.as_str().eq_ignore_ascii_case("deno-notebook-cell")
      && !scheme.as_str().eq_ignore_ascii_case("vscode-userdata")
    {
      return None;
    }
    let mut s = String::with_capacity(uri.as_str().len());
    s.push_str("file:///");
    s.push_str(uri.path().as_str().trim_start_matches('/'));
    if let Some(query) = uri.query() {
      s.push('?');
      s.push_str(query.as_str());
    }
    if let Some(fragment) = uri.fragment() {
      s.push('#');
      s.push_str(fragment.as_str());
    }
    Url::parse(&s).ok().map(normalize_url)
  })()
  .unwrap_or_else(|| normalize_url(Url::parse(uri.as_str()).unwrap()))
}

pub fn uri_to_file_path(uri: &Uri) -> Result<PathBuf, UrlToFilePathError> {
  url_to_file_path(&uri_to_url(uri))
}

pub fn uri_is_file_like(uri: &Uri) -> bool {
  let scheme = uri.scheme();
  scheme.as_str().eq_ignore_ascii_case("file")
    || scheme.as_str().eq_ignore_ascii_case("untitled")
    || scheme.as_str().eq_ignore_ascii_case("vscode-notebook-cell")
    || scheme.as_str().eq_ignore_ascii_case("deno-notebook-cell")
    || scheme.as_str().eq_ignore_ascii_case("vscode-userdata")
}

fn normalize_url(url: Url) -> Url {
  let Ok(path) = url_to_file_path(&url) else {
    return url;
  };
  let normalized_path = normalize_path(&path);
  let Ok(mut normalized_url) = Url::from_file_path(&normalized_path) else {
    return url;
  };
  if let Some(query) = url.query() {
    normalized_url.set_query(Some(query));
  }
  if let Some(fragment) = url.fragment() {
    normalized_url.set_fragment(Some(fragment));
  }
  normalized_url
}

// TODO(nayeemrmn): Change the version of this in deno_path_util to force
// uppercase on drive letters. Then remove this.
fn normalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
  fn inner(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret =
      if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        let s = c.as_os_str();
        if s.len() == 2 {
          PathBuf::from(s.to_ascii_uppercase())
        } else {
          PathBuf::from(s)
        }
      } else {
        PathBuf::new()
      };

    for component in components {
      match component {
        Component::Prefix(..) => unreachable!(),
        Component::RootDir => {
          ret.push(component.as_os_str());
        }
        Component::CurDir => {}
        Component::ParentDir => {
          ret.pop();
        }
        Component::Normal(c) => {
          ret.push(c);
        }
      }
    }
    ret
  }

  inner(path.as_ref())
}
