// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;

use ipnetwork::IpNetwork;
use ipnetwork::Ipv4Network;
use ipnetwork::Ipv6Network;
use serde::Serialize;

#[derive(Serialize)]
struct SocketAddressSerialization(String, String);

#[op2(fast)]
pub fn op_socket_address_parse(
  state: &mut OpState,
  #[string] addr: &str,
  #[smi] port: u16,
  #[string] family: &str,
) -> Result<bool, AnyError> {
  let ip = addr.parse::<IpAddr>()?;
  let parsed: SocketAddr = SocketAddr::new(ip, port);
  let parsed_ip_str = parsed.ip().to_string();
  let family_correct = family.eq_ignore_ascii_case("ipv4") && parsed.is_ipv4()
    || family.eq_ignore_ascii_case("ipv6") && parsed.is_ipv6();

  if family_correct {
    let family_is_lowercase = family[..3].chars().all(char::is_lowercase);
    if family_is_lowercase && parsed_ip_str == addr {
      Ok(true)
    } else {
      state.put::<SocketAddressSerialization>(SocketAddressSerialization(
        parsed_ip_str,
        family.to_lowercase(),
      ));
      Ok(false)
    }
  } else {
    Err(anyhow!("Invalid address"))
  }
}

#[op2]
#[serde]
pub fn op_socket_address_get_serialization(
  state: &mut OpState,
) -> Result<SocketAddressSerialization, AnyError> {
  Ok(state.take::<SocketAddressSerialization>())
}

#[op2]
#[cppgc]
pub fn op_blocklist_new() -> BlockList {
  BlockList::new()
}

#[op2(fast)]
pub fn op_blocklist_add_address(
  #[cppgc] blocklist: &mut BlockList,
  #[string] addr: &str,
) -> Result<(), AnyError> {
  blocklist.add_address(addr)
}

#[op2(fast)]
pub fn op_blocklist_add_range(
  #[cppgc] blocklist: &mut BlockList,
  #[string] start: &str,
  #[string] end: &str,
) -> Result<bool, AnyError> {
  blocklist.add_range(start, end)
}

#[op2(fast)]
pub fn op_blocklist_add_subnet(
  #[cppgc] blocklist: &mut BlockList,
  #[string] addr: &str,
  #[smi] prefix: u8,
) -> Result<(), AnyError> {
  blocklist.add_subnet(addr, prefix)
}

#[op2(fast)]
pub fn op_blocklist_check(
  #[cppgc] blocklist: &BlockList,
  #[string] addr: &str,
  #[string] type_: &str,
) -> Result<bool, AnyError> {
  blocklist.check(addr, type_)
}

struct BlockList {
  rules: HashSet<IpNetwork>,
}

impl deno_core::GcResource for BlockList {}

impl BlockList {
  pub fn new() -> Self {
    BlockList {
      rules: HashSet::new(),
    }
  }

  fn map_addr_add_network(&mut self, addr: IpAddr, prefix: Option<u8>) {
    match addr {
      IpAddr::V4(addr) => {
        self.rules.insert(IpNetwork::V4(
          Ipv4Network::new(addr, prefix.unwrap_or(32)).unwrap(),
        ));
        self.rules.insert(IpNetwork::V6(
          Ipv6Network::new(addr.to_ipv6_mapped(), prefix.unwrap_or(128))
            .unwrap(),
        ));
      }
      IpAddr::V6(addr) => {
        if let Some(ipv4_mapped) = addr.to_ipv4_mapped() {
          self.rules.insert(IpNetwork::V4(
            Ipv4Network::new(ipv4_mapped, prefix.unwrap_or(32)).unwrap(),
          ));
        }
        self.rules.insert(IpNetwork::V6(
          Ipv6Network::new(addr, prefix.unwrap_or(128)).unwrap(),
        ));
      }
    };
  }

  pub fn add_address(&mut self, address: &str) -> Result<(), AnyError> {
    let ip: IpAddr = address.parse()?;
    self.map_addr_add_network(ip, None);
    Ok(())
  }

  pub fn add_range(
    &mut self,
    start: &str,
    end: &str,
  ) -> Result<bool, AnyError> {
    let start_ip: IpAddr = start.parse()?;
    let end_ip: IpAddr = end.parse()?;

    match (start_ip, end_ip) {
      (IpAddr::V4(start), IpAddr::V4(end)) => {
        let start_u32: u32 = start.into();
        let end_u32: u32 = end.into();
        if end_u32 < start_u32 {
          // Indicates invalid range.
          return Ok(false);
        }
        for ip in start_u32..=end_u32 {
          let addr: Ipv4Addr = ip.into();
          self.map_addr_add_network(IpAddr::V4(addr), None);
        }
      }
      (IpAddr::V6(start), IpAddr::V6(end)) => {
        let start_u128: u128 = start.into();
        let end_u128: u128 = end.into();
        if end_u128 < start_u128 {
          // Indicates invalid range.
          return Ok(false);
        }
        for ip in start_u128..=end_u128 {
          let addr: Ipv6Addr = ip.into();
          self.map_addr_add_network(IpAddr::V6(addr), None);
        }
      }
      _ => bail!("IP version mismatch between start and end addresses"),
    }
    Ok(true)
  }

  pub fn add_subnet(&mut self, addr: &str, prefix: u8) -> Result<(), AnyError> {
    let ip: IpAddr = addr.parse()?;
    self.map_addr_add_network(ip, Some(prefix));
    Ok(())
  }

  pub fn check(&self, addr: &str, r#type: &str) -> Result<bool, AnyError> {
    let addr: IpAddr = addr.parse()?;
    let family = r#type.to_lowercase();
    if family == "ipv4" && addr.is_ipv4() || family == "ipv6" && addr.is_ipv6()
    {
      Ok(self.rules.iter().any(|net| net.contains(addr)))
    } else {
      Err(anyhow!("Invalid address"))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_add_address() {
    // Single IPv4 address
    let mut block_list = BlockList::new();
    block_list.add_address("192.168.0.1").unwrap();
    assert!(block_list.check("192.168.0.1", "ipv4").unwrap());
    assert!(block_list.check("::ffff:c0a8:1", "ipv6").unwrap());

    // Single IPv6 address
    let mut block_list = BlockList::new();
    block_list.add_address("2001:db8::1").unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6").unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4").unwrap());
  }

  #[test]
  fn test_add_range() {
    // IPv4 range
    let mut block_list = BlockList::new();
    block_list.add_range("192.168.0.1", "192.168.0.3").unwrap();
    assert!(block_list.check("192.168.0.1", "ipv4").unwrap());
    assert!(block_list.check("192.168.0.2", "ipv4").unwrap());
    assert!(block_list.check("192.168.0.3", "ipv4").unwrap());
    assert!(block_list.check("::ffff:c0a8:1", "ipv6").unwrap());

    // IPv6 range
    let mut block_list = BlockList::new();
    block_list.add_range("2001:db8::1", "2001:db8::3").unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6").unwrap());
    assert!(block_list.check("2001:db8::2", "ipv6").unwrap());
    assert!(block_list.check("2001:db8::3", "ipv6").unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4").unwrap());
  }

  #[test]
  fn test_add_subnet() {
    // IPv4 subnet
    let mut block_list = BlockList::new();
    block_list.add_subnet("192.168.0.0", 24).unwrap();
    assert!(block_list.check("192.168.0.1", "ipv4").unwrap());
    assert!(block_list.check("192.168.0.255", "ipv4").unwrap());
    assert!(block_list.check("::ffff:c0a8:0", "ipv6").unwrap());

    // IPv6 subnet
    let mut block_list = BlockList::new();
    block_list.add_subnet("2001:db8::", 64).unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6").unwrap());
    assert!(block_list.check("2001:db8::ffff", "ipv6").unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4").unwrap());
  }

  #[test]
  fn test_check() {
    // Check IPv4 presence
    let mut block_list = BlockList::new();
    block_list.add_address("192.168.0.1").unwrap();
    assert!(block_list.check("192.168.0.1", "ipv4").unwrap());

    // Check IPv6 presence
    let mut block_list = BlockList::new();
    block_list.add_address("2001:db8::1").unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6").unwrap());

    // Check IPv4 not present
    let block_list = BlockList::new();
    assert!(!block_list.check("192.168.0.1", "ipv4").unwrap());

    // Check IPv6 not present
    let block_list = BlockList::new();
    assert!(!block_list.check("2001:db8::1", "ipv6").unwrap());

    // Check invalid IP version
    let block_list = BlockList::new();
    assert!(block_list.check("192.168.0.1", "ipv6").is_err());

    // Check invalid type
    let mut block_list = BlockList::new();
    block_list.add_address("192.168.0.1").unwrap();
    assert!(block_list.check("192.168.0.1", "invalid_type").is_err());
  }
}
