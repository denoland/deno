// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;

use deno_core::OpState;
use deno_permissions::PermissionCheckError;
use tokio::net::lookup_host;

use crate::NetPermissions;
use crate::ops::NetError;

/// Resolve network address *asynchronously*.
async fn resolve_addr(
  hostname: &str,
  port: u16,
) -> Result<impl Iterator<Item = SocketAddr> + '_, std::io::Error> {
  let addr_port_pair = make_addr_port_pair(hostname, port);
  let result = lookup_host(addr_port_pair).await?;
  Ok(result)
}

/// Resolve network address *synchronously*.
pub fn resolve_addr_sync_i_promise_i_dont_need_permissions(
  hostname: &str,
  port: u16,
) -> Result<impl Iterator<Item = SocketAddr> + use<>, std::io::Error> {
  let addr_port_pair = make_addr_port_pair(hostname, port);
  let result = addr_port_pair.to_socket_addrs()?;
  Ok(result)
}

fn make_addr_port_pair(hostname: &str, port: u16) -> (&str, u16) {
  // Default to localhost if given just the port. Example: ":80"
  if hostname.is_empty() {
    return ("0.0.0.0", port);
  }

  // If this looks like an ipv6 IP address. Example: "[2001:db8::1]"
  // Then we remove the brackets.
  let addr = hostname.trim_start_matches('[').trim_end_matches(']');
  (addr, port)
}

pub async fn resolve_addr_with_permissions<NP>(
  state: &RefCell<OpState>,
  api_name: &str,
  addr: crate::ops::IpAddr,
) -> Result<SocketAddr, NetError>
where
  NP: NetPermissions + 'static,
{
  let mut addrs = resolve_addr(&addr.hostname, addr.port).await?;
  let addr = addrs.next().ok_or_else(|| NetError::NoResolvedAddress)?;
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<NP>();
    permissions.check_net_resolved_addr_is_not_denied(&addr, api_name)?;
    for addr in addrs {
      permissions.check_net_resolved_addr_is_not_denied(&addr, api_name)?;
    }
  }
  Ok(addr)
}

pub fn resolve_addr_sync_with_permissions<NP, ErrorType>(
  state: &mut OpState,
  api_name: &str,
  addr: &crate::ops::IpAddr,
) -> Result<SocketAddr, ErrorType>
where
  NP: NetPermissions + 'static,
  ErrorType: DnsError,
{
  let mut addrs = resolve_addr_sync_i_promise_i_dont_need_permissions(
    &addr.hostname,
    addr.port,
  )?;
  let addr = addrs
    .next()
    .ok_or_else(|| ErrorType::no_resolved_address())?;
  {
    let permissions = state.borrow_mut::<NP>();
    permissions.check_net_resolved_addr_is_not_denied(&addr, api_name)?;
    for addr in addrs {
      permissions.check_net_resolved_addr_is_not_denied(&addr, api_name)?;
    }
  }
  Ok(addr)
}

pub trait DnsError: From<PermissionCheckError> + From<std::io::Error> {
  fn no_resolved_address() -> Self;
}

impl DnsError for NetError {
  fn no_resolved_address() -> Self {
    NetError::NoResolvedAddress
  }
}

impl DnsError for crate::quic::QuicError {
  fn no_resolved_address() -> Self {
    crate::quic::QuicError::UnableToResolve
  }
}

#[cfg(test)]
mod tests {
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::net::SocketAddrV4;
  use std::net::SocketAddrV6;

  use super::resolve_addr_sync_i_promise_i_dont_need_permissions as resolve_addr_sync;
  use super::*;

  #[tokio::test]
  async fn resolve_addr1() {
    let expected = vec![SocketAddr::V4(SocketAddrV4::new(
      Ipv4Addr::new(127, 0, 0, 1),
      80,
    ))];
    let actual = resolve_addr("127.0.0.1", 80)
      .await
      .unwrap()
      .collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr2() {
    let expected = vec![SocketAddr::V4(SocketAddrV4::new(
      Ipv4Addr::new(0, 0, 0, 0),
      80,
    ))];
    let actual = resolve_addr("", 80).await.unwrap().collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr3() {
    let expected = vec![SocketAddr::V4(SocketAddrV4::new(
      Ipv4Addr::new(192, 0, 2, 1),
      25,
    ))];
    let actual = resolve_addr("192.0.2.1", 25)
      .await
      .unwrap()
      .collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr_ipv6() {
    let expected = vec![SocketAddr::V6(SocketAddrV6::new(
      Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
      8080,
      0,
      0,
    ))];
    let actual = resolve_addr("[2001:db8::1]", 8080)
      .await
      .unwrap()
      .collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr_err() {
    assert!(resolve_addr("INVALID ADDR", 1234).await.is_err());
  }

  #[test]
  fn resolve_addr_sync1() {
    let expected = vec![SocketAddr::V4(SocketAddrV4::new(
      Ipv4Addr::new(127, 0, 0, 1),
      80,
    ))];
    let actual = resolve_addr_sync("127.0.0.1", 80)
      .unwrap()
      .collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync2() {
    let expected = vec![SocketAddr::V4(SocketAddrV4::new(
      Ipv4Addr::new(0, 0, 0, 0),
      80,
    ))];
    let actual = resolve_addr_sync("", 80).unwrap().collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync3() {
    let expected = vec![SocketAddr::V4(SocketAddrV4::new(
      Ipv4Addr::new(192, 0, 2, 1),
      25,
    ))];
    let actual = resolve_addr_sync("192.0.2.1", 25)
      .unwrap()
      .collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync_ipv6() {
    let expected = vec![SocketAddr::V6(SocketAddrV6::new(
      Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
      8080,
      0,
      0,
    ))];
    let actual = resolve_addr_sync("[2001:db8::1]", 8080)
      .unwrap()
      .collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync_err() {
    assert!(resolve_addr_sync("INVALID ADDR", 1234).is_err());
  }
}
