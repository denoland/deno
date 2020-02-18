// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![allow(unused)]

use deno_core::ErrBox;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::path::Path;
use url::Url;

/// Turn base of url (scheme, hostname, port) into a valid filename.
/// This method replaces port part with a special string token (because
/// ":" cannot be used in filename on some platforms).
pub fn base_url_to_filename(url: &Url) -> PathBuf {
  let mut out = PathBuf::new();

  let scheme = url.scheme();
  out.push(scheme);

  match scheme {
    "http" | "https" => {
      let host = url.host_str().unwrap();
      let host_port = match url.port() {
        Some(port) => format!("{}_PORT{}", host, port),
        None => host.to_string(),
      };
      out.push(host_port);
    }
    scheme => {
      unimplemented!(
        "Don't know how to create cache name for scheme: {}",
        scheme
      );
    }
  };

  out
}

/// Turn provided `url` into a hashed filename.
/// URLs can contain a lot of characters that cannot be used 
/// in filenames (like "?", "#", ":"), so in order to cache
/// them properly they are deterministically hashed into ASCII
/// strings.
fn url_to_filename(url: &Url) -> PathBuf {
  let hostname_filename = base_url_to_filename(url);

  let mut rest_str = url.path().to_string();
  if let Some(query) = url.query() {
    rest_str.push_str("?");
    rest_str.push_str(query);
  }
  // NOTE: fragment is omitted on purpose - it's not taken into
  // account when caching - it denotes parts of webpage, which
  // in case of static resources doesn't make much sense
  let hashed_filename = crate::checksum::gen(vec![rest_str.as_bytes()]);
  let mut cache_filename = PathBuf::from(hostname_filename);
  cache_filename.push(hashed_filename);
  cache_filename
}

pub type HeadersMap = HashMap<String, String>;

pub struct HttpCache {
  pub location: PathBuf,
}

impl HttpCache {
  /// Returns error if unable to create directory 
  /// at specified location.
  pub fn new(location: &Path) -> Result<Self, ErrBox> {
    fs::create_dir_all(&location)?;
    Ok(Self {
      location: location.to_owned()
    })
  }

  pub fn get(&self, url: &Url) -> Option<(File, HeadersMap)> {
    todo!()
  }

  pub fn set(
    &self,
    url: &Url,
    headers_map: HeadersMap,
    content: &[u8],
  ) -> Result<(), ErrBox> {
    todo!()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_create_cache() {
    let dir = TempDir::new().unwrap();
    let mut cache_path = dir.path().to_owned();
    cache_path.push("foobar");
    let r = HttpCache::new(&cache_path);
    assert!(r.is_ok());
    assert!(cache_path.is_dir());
  }

  #[test]
  fn test_url_to_filename() {
    let test_cases = [
      ("https://deno.land/x/foo.ts", "https/deno.land/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8"),
      (
        "https://deno.land:8080/x/foo.ts",
        "https/deno.land_PORT8080/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8",
      ),
      ("https://deno.land/", "https/deno.land/8a5edab282632443219e051e4ade2d1d5bbc671c781051bf1437897cbdfea0f1"),
      (
        "https://deno.land/?asdf=qwer",
        "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
      ),
      // should be the same as case above, fragment (#qwer) is ignored
      // when hashing
      (
        "https://deno.land/?asdf=qwer#qwer",
        "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
      ),
    ];

    for (url, expected) in test_cases.iter() {
      let u = Url::parse(url).unwrap();
      let p = url_to_filename(&u);
      assert_eq!(p, PathBuf::from(expected));
    }
  }
}
