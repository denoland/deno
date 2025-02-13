// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use deno_core::op2;
use deno_core::OpState;
use deno_error::JsError;
use deno_net::ops::NetPermToken;
use deno_permissions::PermissionCheckError;
use hyper_util::client::legacy::connect::dns::GaiResolver;
use hyper_util::client::legacy::connect::dns::Name;
use tower_service::Service;

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
#[cppgc]
pub async fn op_node_getaddrinfo<P>(
  state: Rc<RefCell<OpState>>,
  #[string] hostname: String,
  port: Option<u16>,
) -> Result<NetPermToken, GetAddrInfoError>
where
  P: crate::NodePermissions + 'static,
{
  {
    let mut state_ = state.borrow_mut();
    let permissions = state_.borrow_mut::<P>();
    permissions.check_net((hostname.as_str(), port), "node:dns.lookup()")?;
  }

  let mut resolver = GaiResolver::new();
  let name = Name::from_str(&hostname)
    .map_err(|_| GetAddrInfoError::Resolution(hostname.clone()))?;
  let resolved_ips = resolver
    .call(name)
    .await
    .map_err(|_| GetAddrInfoError::Resolution(hostname.clone()))?
    .map(|addr| addr.ip().to_string())
    .collect::<Vec<_>>();
  Ok(NetPermToken {
    hostname,
    port,
    resolved_ips,
  })
}
