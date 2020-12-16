// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_fetch::reqwest;
use deno_fetch::reqwest::header::HeaderMap;
use deno_fetch::reqwest::header::USER_AGENT;
use deno_fetch::reqwest::redirect::Policy;
use deno_fetch::reqwest::Client;
use std::fs::File;
use std::io::Read;

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client(
  user_agent: String,
  ca_file: Option<&str>,
) -> Result<Client, AnyError> {
  let mut headers = HeaderMap::new();
  headers.insert(USER_AGENT, user_agent.parse().unwrap());
  let mut builder = Client::builder()
    .redirect(Policy::none())
    .default_headers(headers)
    .use_rustls_tls();

  if let Some(ca_file) = ca_file {
    let mut buf = Vec::new();
    File::open(ca_file)?.read_to_end(&mut buf)?;
    let cert = reqwest::Certificate::from_pem(&buf)?;
    builder = builder.add_root_certificate(cert);
  }

  builder
    .build()
    .map_err(|_| generic_error("Unable to build http client"))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn create_test_client() {
    create_http_client("test_client".to_string(), None).unwrap();
  }
}
