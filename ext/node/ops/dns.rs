// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use deno_core::op2;
use deno_core::OpState;
use deno_error::JsError;
use deno_permissions::PermissionCheckError;
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

#[derive(Debug, thiserror::Error, JsError)]
pub enum GetAddrInfoError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
  #[class(type)]
  #[error("Could not resolve the hostname \"{0}\"")]
  Resolution(String),
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_getaddrinfo<P>(
  state: Rc<RefCell<OpState>>,
  #[string] hostname: String,
  port: Option<u16>,
) -> Result<Vec<GetAddrInfoResult>, GetAddrInfoError>
where
  P: crate::NodePermissions + 'static,
{
  {
    let mut state_ = state.borrow_mut();
    let permissions = state_.borrow_mut::<P>();
    permissions.check_net((hostname.as_str(), port), "lookup")?;
  }
  let mut resolver = GaiResolver::new();
  let name = Name::from_str(&hostname)
    .map_err(|_| GetAddrInfoError::Resolution(hostname.clone()))?;
  resolver
    .call(name)
    .await
    .map_err(|_| GetAddrInfoError::Resolution(hostname))
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
