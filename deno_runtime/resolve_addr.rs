// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;

/// Resolve network address. Returns a future.
pub fn resolve_addr(hostname: &str, port: u16) -> Result<SocketAddr, AnyError> {
  // Default to localhost if given just the port. Example: ":80"
  let addr: &str = if !hostname.is_empty() {
    &hostname
  } else {
    "0.0.0.0"
  };

  // If this looks like an ipv6 IP address. Example: "[2001:db8::1]"
  // Then we remove the brackets.
  let addr = if addr.starts_with('[') && addr.ends_with(']') {
    let l = addr.len() - 1;
    addr.get(1..l).unwrap()
  } else {
    addr
  };
  let addr_port_pair = (addr, port);
  let mut iter = addr_port_pair.to_socket_addrs()?;
  Ok(iter.next().unwrap())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::net::SocketAddrV4;
  use std::net::SocketAddrV6;

  #[test]
  fn resolve_addr1() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 80));
    let actual = resolve_addr("127.0.0.1", 80).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr2() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 80));
    let actual = resolve_addr("", 80).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr3() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 25));
    let actual = resolve_addr("192.0.2.1", 25).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_ipv6() {
    let expected = SocketAddr::V6(SocketAddrV6::new(
      Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
      8080,
      0,
      0,
    ));
    let actual = resolve_addr("[2001:db8::1]", 8080).unwrap();
    assert_eq!(actual, expected);
  }
}
