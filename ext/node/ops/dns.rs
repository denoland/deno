// Copyright 2018-2025 the Deno authors. MIT license.

use std::str::FromStr;

use deno_core::op2;
use hyper_util::client::legacy::connect::dns::GaiResolver;
use hyper_util::client::legacy::connect::dns::Name;
use serde::Serialize;
use tower_service::Service;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetAddrInfoResult {
  family: usize,
  address: String,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Could not resolve the hostname '{hostname}'")]
pub struct GetAddrInfoError {
  hostname: String,
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_getaddrinfo(
  #[string] hostname: String,
) -> Result<Vec<GetAddrInfoResult>, GetAddrInfoError> {
  let mut resolver = GaiResolver::new();
  let name = Name::from_str(&hostname).map_err(|_| GetAddrInfoError {
    hostname: hostname.clone(),
  })?;
  resolver
    .call(name)
    .await
    .map_err(|_| GetAddrInfoError { hostname })
    .map(|addrs| {
      addrs
        .into_iter()
        .map(|addr| GetAddrInfoResult {
          family: match addr {
            std::net::SocketAddr::V4(_) => 4,
            std::net::SocketAddr::V6(_) => 6,
          },
          address: addr.ip().to_string(),
        })
        .collect::<Vec<_>>()
    })
}
