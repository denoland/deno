// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Error as IoError;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Default timeout for each connection attempt (matches Node.js default)
pub const DEFAULT_ATTEMPT_TIMEOUT_MS: u64 = 250;

/// Result of a Happy Eyeballs connection attempt
#[derive(Debug)]
pub struct HappyEyeballsResult {
  pub stream: TcpStream,
  pub addr: SocketAddr,
  pub attempted_addresses: Vec<String>,
}

/// Interleave IPv6 and IPv4 addresses per RFC 8305 Section 4.
///
/// The algorithm prefers IPv6, so we alternate: first IPv6, first IPv4,
/// second IPv6, second IPv4, etc.
///
/// Input:  `[v6_1, v6_2, v4_1, v4_2]`
/// Output: `[v6_1, v4_1, v6_2, v4_2]`
fn interleave_addresses(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
  let (v6, v4): (Vec<_>, Vec<_>) = addrs.into_iter().partition(|a| a.is_ipv6());

  let mut result = Vec::with_capacity(v6.len() + v4.len());
  let mut v6_iter = v6.into_iter();
  let mut v4_iter = v4.into_iter();

  loop {
    match (v6_iter.next(), v4_iter.next()) {
      (Some(v6), Some(v4)) => {
        result.push(v6);
        result.push(v4);
      }
      (Some(v6), None) => result.push(v6),
      (None, Some(v4)) => result.push(v4),
      (None, None) => break,
    }
  }

  result
}

/// Connect using Happy Eyeballs algorithm (RFC 8305 - sequential variant).
///
/// This implementation uses sequential connection attempts with timeouts,
/// similar to Node.js's approach. Each address is tried in order with a
/// short timeout before moving to the next.
///
/// See also: <https://github.com/nodejs/node/issues/48145>
pub async fn connect_happy_eyeballs(
  addrs: Vec<SocketAddr>,
  attempt_timeout: Duration,
  cancel_handle: Option<Rc<CancelHandle>>,
) -> Result<HappyEyeballsResult, IoError> {
  if addrs.is_empty() {
    return Err(IoError::new(
      ErrorKind::InvalidInput,
      "No addresses to connect to",
    ));
  }

  let addrs = interleave_addresses(addrs);
  let mut last_error = None;
  let mut attempted_addresses = Vec::new();

  for (i, addr) in addrs.iter().enumerate() {
    attempted_addresses.push(addr.to_string());

    let is_last = i == addrs.len() - 1;

    let connect_result = if is_last {
      // Last address: no timeout, let it complete or fail naturally
      connect_with_cancel(*addr, cancel_handle.as_ref()).await
    } else {
      // Non-last addresses: use timeout, move to next on timeout
      match timeout(
        attempt_timeout,
        connect_with_cancel(*addr, cancel_handle.as_ref()),
      )
      .await
      {
        Ok(result) => result,
        Err(_) => continue,
      }
    };

    match connect_result {
      Ok(stream) => {
        return Ok(HappyEyeballsResult {
          stream,
          addr: *addr,
          attempted_addresses,
        });
      }
      Err(e) => {
        last_error = Some(e);
      }
    }
  }

  // last_error is always Some here because:
  // - addrs is non-empty (checked above)
  // - the last address has no timeout and always sets last_error on failure
  Err(last_error.expect("last_error should be set after iterating non-empty addrs"))
}

/// Connect to a single address with optional cancellation support.
async fn connect_with_cancel(
  addr: SocketAddr,
  cancel_handle: Option<&Rc<CancelHandle>>,
) -> Result<TcpStream, IoError> {
  if let Some(cancel) = cancel_handle {
    TcpStream::connect(addr)
      .or_cancel(cancel)
      .await
      .map_err(|_| {
        IoError::new(ErrorKind::Interrupted, "Connection cancelled")
      })?
  } else {
    TcpStream::connect(addr).await
  }
}

#[cfg(test)]
mod tests {
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::net::SocketAddrV4;
  use std::net::SocketAddrV6;

  use super::*;

  #[test]
  fn test_interleave_empty() {
    let result = interleave_addresses(vec![]);
    assert!(result.is_empty());
  }

  #[test]
  fn test_interleave_single_v4() {
    let v4 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80));
    let result = interleave_addresses(vec![v4]);
    assert_eq!(result, vec![v4]);
  }

  #[test]
  fn test_interleave_single_v6() {
    let v6 = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0));
    let result = interleave_addresses(vec![v6]);
    assert_eq!(result, vec![v6]);
  }

  #[test]
  fn test_interleave_one_v6_one_v4() {
    let v6 = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0));
    let v4 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80));

    // Input order: v4 first, then v6
    let result = interleave_addresses(vec![v4, v6]);

    // Expected: v6 first (IPv6 preferred), then v4
    assert_eq!(result, vec![v6, v4]);
  }

  #[test]
  fn test_interleave_balanced() {
    let v6_1 = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0));
    let v6_2 = SocketAddr::V6(SocketAddrV6::new(
      Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
      80,
      0,
      0,
    ));
    let v4_1 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80));
    let v4_2 =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 80));

    let result = interleave_addresses(vec![v6_1, v6_2, v4_1, v4_2]);

    // Expected: v6_1, v4_1, v6_2, v4_2
    assert_eq!(result.len(), 4);
    assert!(result[0].is_ipv6());
    assert!(result[1].is_ipv4());
    assert!(result[2].is_ipv6());
    assert!(result[3].is_ipv4());
  }

  #[test]
  fn test_interleave_more_v6() {
    let v6_1 = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0));
    let v6_2 = SocketAddr::V6(SocketAddrV6::new(
      Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
      80,
      0,
      0,
    ));
    let v4_1 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80));

    let result = interleave_addresses(vec![v6_1, v6_2, v4_1]);

    // Expected: v6_1, v4_1, v6_2
    assert_eq!(result.len(), 3);
    assert!(result[0].is_ipv6());
    assert!(result[1].is_ipv4());
    assert!(result[2].is_ipv6());
  }

  #[test]
  fn test_interleave_more_v4() {
    let v6_1 = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0));
    let v4_1 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80));
    let v4_2 =
      SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 80));

    let result = interleave_addresses(vec![v6_1, v4_1, v4_2]);

    // Expected: v6_1, v4_1, v4_2
    assert_eq!(result.len(), 3);
    assert!(result[0].is_ipv6());
    assert!(result[1].is_ipv4());
    assert!(result[2].is_ipv4());
  }

  #[tokio::test]
  async fn test_connect_empty_addresses() {
    let result =
      connect_happy_eyeballs(vec![], Duration::from_millis(250), None).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
  }

  #[tokio::test]
  async fn test_connect_single_address_succeeds() {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let result =
      connect_happy_eyeballs(vec![addr], Duration::from_millis(250), None)
        .await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.addr, addr);
    assert_eq!(result.attempted_addresses, vec![addr.to_string()]);
  }

  /// Returns a socket address that will refuse connections.
  /// Note: There's a small race window where another process could bind
  /// to this port, but it's acceptable for test purposes.
  async fn get_refusing_addr() -> SocketAddr {
    use tokio::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr
  }

  #[tokio::test]
  async fn test_connect_single_address_fails() {
    let bad_addr = get_refusing_addr().await;

    let result =
      connect_happy_eyeballs(vec![bad_addr], Duration::from_millis(100), None)
        .await;

    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_connect_fallback_to_second() {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let good_addr = listener.local_addr().unwrap();
    let bad_addr = get_refusing_addr().await;

    let result = connect_happy_eyeballs(
      vec![bad_addr, good_addr],
      Duration::from_millis(250),
      None,
    )
    .await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.addr, good_addr);
    assert_eq!(result.attempted_addresses.len(), 2);
  }

  #[tokio::test]
  async fn test_connect_all_fail() {
    let bad1 = get_refusing_addr().await;
    let bad2 = get_refusing_addr().await;

    let result = connect_happy_eyeballs(
      vec![bad1, bad2],
      Duration::from_millis(100),
      None,
    )
    .await;

    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_connect_timeout_moves_to_next() {
    use tokio::net::TcpListener;

    // Use a non-routable IP that will hang (black hole)
    let hanging_addr: SocketAddr = "10.255.255.1:80".parse().unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let good_addr = listener.local_addr().unwrap();

    let start = std::time::Instant::now();
    let result = connect_happy_eyeballs(
      vec![hanging_addr, good_addr],
      Duration::from_millis(100), // Short timeout
      None,
    )
    .await;

    let elapsed = start.elapsed();

    assert!(result.is_ok());
    assert_eq!(result.unwrap().addr, good_addr);
    // Should complete quickly (timeout + connection), not hang for seconds
    assert!(elapsed < Duration::from_secs(2));
  }

  #[tokio::test]
  async fn test_connect_cancellation() {
    use tokio::net::TcpListener;

    let cancel = CancelHandle::new_rc();
    cancel.cancel();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let result = connect_happy_eyeballs(
      vec![addr],
      Duration::from_millis(250),
      Some(cancel),
    )
    .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::Interrupted);
  }
}
