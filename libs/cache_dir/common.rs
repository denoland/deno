// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;

use url::Url;

// TODO(ry) HTTP headers are not unique key, value pairs. There may be more than
// one header line with the same key. This should be changed to something like
// Vec<(String, String)>
pub type HeadersMap = HashMap<String, String>;

// Both forms intentionally differ from the legacy underscore-based
// representation: an old entry may be ambiguous, so it must be regenerated.
const PORT_SEPARATOR: &str = "_port_";
const UNDERSCORE_ESCAPE: &str = "%5f";

pub fn base_url_to_filename_parts(url: &Url) -> Option<Vec<Cow<'_, str>>> {
  let mut out = Vec::with_capacity(2);

  let scheme = url.scheme();

  match scheme {
    "http" | "https" => {
      out.push(Cow::Borrowed(scheme));

      let host = url.host_str().unwrap();
      // URL hosts may contain underscores, so reserve underscores in the
      // filename representation for the port separator.
      let host = if host.contains('_') {
        Cow::Owned(host.replace('_', UNDERSCORE_ESCAPE))
      } else {
        Cow::Borrowed(host)
      };
      let host_port = match url.port() {
        Some(port) => Cow::Owned(format!("{host}{PORT_SEPARATOR}{port}")),
        None => host,
      };
      out.push(host_port);
    }
    "data" | "blob" => {
      out.push(Cow::Borrowed(scheme));
    }
    scheme => {
      log::debug!("Don't know how to create cache name for scheme: {}", scheme);
      return None;
    }
  };

  Some(out)
}

pub fn filename_part_to_url_authority(part: &str) -> Option<String> {
  let (host, port) =
    if let Some((host, port)) = part.rsplit_once(PORT_SEPARATOR) {
      (host, Some(port.parse::<u16>().ok()?))
    } else {
      (part, None)
    };

  // Raw underscores belong to the old ambiguous representation. New
  // filename parts always escape host underscores.
  if host.contains('_') {
    return None;
  }
  let host = host.replace(UNDERSCORE_ESCAPE, "_");
  Some(match port {
    Some(port) => format!("{host}:{port}"),
    None => host,
  })
}

pub fn checksum(v: &[u8]) -> String {
  use sha2::Digest;
  use sha2::Sha256;

  let mut hasher = Sha256::new();
  hasher.update(v);
  format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_gen() {
    let actual = checksum(b"hello world");
    assert_eq!(
      actual,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }

  #[test]
  fn test_base_url_authority_filename_parts() {
    let underscore_host = Url::parse("http://api_8080/mod.ts").unwrap();
    let explicit_port = Url::parse("http://api:8080/mod.ts").unwrap();

    assert_eq!(
      base_url_to_filename_parts(&underscore_host).unwrap(),
      ["http", "api%5f8080"]
    );
    assert_eq!(
      base_url_to_filename_parts(&explicit_port).unwrap(),
      ["http", "api_port_8080"]
    );
    assert_eq!(
      filename_part_to_url_authority("api%5f8080").as_deref(),
      Some("api_8080")
    );
    assert_eq!(
      filename_part_to_url_authority("api_port_8080").as_deref(),
      Some("api:8080")
    );
    assert_eq!(filename_part_to_url_authority("api_8080"), None);
  }
}
