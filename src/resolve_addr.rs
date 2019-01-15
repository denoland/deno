// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

use futures::Async;
use futures::Future;
use futures::Poll;
use std::error::Error;
use std::fmt;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;

/// Go-style network address parsing. Returns a future.
/// Examples:
/// "192.0.2.1:25"
/// ":80"
/// "[2001:db8::1]:80"
/// "198.51.100.1:80"
/// "deno.land:443"
pub fn resolve_addr(address: &str) -> ResolveAddrFuture {
  ResolveAddrFuture {
    address: address.to_string(),
  }
}

#[derive(Debug)]
pub enum ResolveAddrError {
  Syntax,
  Resolution(std::io::Error),
}

impl fmt::Display for ResolveAddrError {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt.write_str(self.description())
  }
}

impl Error for ResolveAddrError {
  fn description(&self) -> &str {
    match self {
      ResolveAddrError::Syntax => "invalid address syntax",
      ResolveAddrError::Resolution(e) => e.description(),
    }
  }
}

pub struct ResolveAddrFuture {
  address: String,
}

impl Future for ResolveAddrFuture {
  type Item = SocketAddr;
  type Error = ResolveAddrError;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    // The implementation of this is not actually async at the moment,
    // however we intend to use async DNS resolution in the future and
    // so we expose this as a future instead of Result.
    match split(&self.address) {
      None => Err(ResolveAddrError::Syntax),
      Some(addr_port_pair) => {
        // I absolutely despise the .to_socket_addrs() API.
        let r = addr_port_pair
          .to_socket_addrs()
          .map_err(ResolveAddrError::Resolution);

        r.and_then(|mut iter| match iter.next() {
          Some(a) => Ok(Async::Ready(a)),
          None => panic!("There should be at least one result"),
        })
      }
    }
  }
}

fn split(address: &str) -> Option<(&str, u16)> {
  address.rfind(':').and_then(|i| {
    let (a, p) = address.split_at(i);
    // Default to localhost if given just the port. Example: ":80"
    let addr = if !a.is_empty() { a } else { "0.0.0.0" };
    // If this looks like an ipv6 IP address. Example: "[2001:db8::1]"
    // Then we remove the brackets.
    let addr = if addr.starts_with('[') && addr.ends_with(']') {
      let l = addr.len() - 1;
      addr.get(1..l).unwrap()
    } else {
      addr
    };

    let p = p.trim_start_matches(':');
    match p.parse::<u16>() {
      Err(_) => None,
      Ok(port) => Some((addr, port)),
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::net::SocketAddrV4;
  use std::net::SocketAddrV6;

  #[test]
  fn split1() {
    assert_eq!(split("127.0.0.1:80"), Some(("127.0.0.1", 80)));
  }

  #[test]
  fn split2() {
    assert_eq!(split(":80"), Some(("0.0.0.0", 80)));
  }

  #[test]
  fn split3() {
    assert_eq!(split("no colon"), None);
  }

  #[test]
  fn split4() {
    assert_eq!(split("deno.land:443"), Some(("deno.land", 443)));
  }

  #[test]
  fn split5() {
    assert_eq!(split("[2001:db8::1]:8080"), Some(("2001:db8::1", 8080)));
  }

  #[test]
  fn resolve_addr1() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 80));
    let actual = resolve_addr("127.0.0.1:80").wait().unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr3() {
    let expected =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 25));
    let actual = resolve_addr("192.0.2.1:25").wait().unwrap();
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
    let actual = resolve_addr("[2001:db8::1]:8080").wait().unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn resolve_addr_err() {
    let r = resolve_addr("not-a-real-domain.blahblah:8080").wait();
    match r {
      Err(ResolveAddrError::Resolution(_)) => {} // expected
      _ => assert!(false),
    }
  }
}
