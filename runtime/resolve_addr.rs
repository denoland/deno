// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::error::Context;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use tokio::net::lookup_host;

/// Resolve network address *asynchronously*.
pub async fn resolve_addr(
  hostname: &str,
  port: u16,
) -> Result<SocketAddr, AnyError> {
  let addr_port_pair = make_addr_port_pair(hostname, port);
  lookup_host(addr_port_pair)
    .await?
    .next()
    .context("No resolved address found")
}

/// Resolve network address *synchronously*.
pub fn resolve_addr_sync(
  hostname: &str,
  port: u16,
) -> Result<SocketAddr, AnyError> {
  let addr_port_pair = make_addr_port_pair(hostname, port);
  addr_port_pair
    .to_socket_addrs()?
    .next()
    .context("No resolved address found")
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

#[cfg(test)]
mod tests {
  use super::*;
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::net::SocketAddrV4;
  use std::net::SocketAddrV6;

  #[tokio::test]
  async fn resolve_addr1() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 80));
    let actual = resolve_addr("127.0.0.1", 80).await.unwrap();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr2() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 80));
    let actual = resolve_addr("", 80).await.unwrap();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr3() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 25));
    let actual = resolve_addr("192.0.2.1", 25).await.unwrap();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr_ipv6() {
    let expected = SocketAddr::V6(SocketAddrV6::new(
      Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
      8080,
      0,
      0,
    ));
    let actual = resolve_addr("[2001:db8::1]", 8080).await.unwrap();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn resolve_addr_err() {
    assert!(resolve_addr("INVALID ADDR", 1234).await.is_err());
  }

  #[test]
  fn resolve_addr_sync1() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 80));
    let actual = resolve_addr_sync("127.0.0.1", 80).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync2() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 80));
    let actual = resolve_addr_sync("", 80).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync3() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 25));
    let actual = resolve_addr_sync("192.0.2.1", 25).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync_ipv6() {
    let expected = SocketAddr::V6(SocketAddrV6::new(
      Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
      8080,
      0,
      0,
    ));
    let actual = resolve_addr_sync("[2001:db8::1]", 8080).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_sync_err() {
    assert!(resolve_addr_sync("INVALID ADDR", 1234).is_err());
  }
}
