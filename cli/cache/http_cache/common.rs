// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_core::url::Url;

pub fn base_url_to_filename_parts(
  url: &Url,
  port_separator: &str,
) -> Option<Vec<String>> {
  let mut out = Vec::with_capacity(2);

  let scheme = url.scheme();
  out.push(scheme.to_string());

  match scheme {
    "http" | "https" => {
      let host = url.host_str().unwrap();
      let host_port = match url.port() {
        // underscores are not allowed in domains, so adding one here is fine
        Some(port) => format!("{host}{port_separator}{port}"),
        None => host.to_string(),
      };
      out.push(host_port);
    }
    "data" | "blob" => (),
    scheme => {
      log::debug!("Don't know how to create cache name for scheme: {}", scheme);
      return None;
    }
  };

  Some(out)
}

pub fn read_file_bytes(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
  match std::fs::read(path) {
    Ok(s) => Ok(Some(s)),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(err) => Err(err),
  }
}

/// Ensures the location of the cache.
pub fn ensure_dir_exists(path: &Path) -> std::io::Result<()> {
  if path.is_dir() {
    return Ok(());
  }
  std::fs::create_dir_all(path).map_err(|e| {
    std::io::Error::new(
      e.kind(),
      format!(
        "Could not create remote modules cache location: {path:?}\nCheck the permission of the directory."
      ),
    )
  })
}
