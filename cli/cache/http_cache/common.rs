// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
