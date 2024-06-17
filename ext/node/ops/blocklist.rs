// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;

use ipnetwork::IpNetwork;
use ipnetwork::Ipv4Network;
use ipnetwork::Ipv6Network;

pub struct BlockListResource {
  blocklist: RefCell<BlockList>,
}

impl Resource for BlockListResource {
  fn name(&self) -> Cow<str> {
    "blocklist".into()
  }
}

#[op2]
#[serde]
pub fn op_socket_address_parse(
  _state: &mut OpState,
  #[string] addr: String,
  #[smi] port: u16,
  #[string] family: String,
) -> Result<(IpAddr, u16, String), AnyError> {
  let ip = addr.parse::<IpAddr>()?;
  let family = family.to_lowercase();
  let port = port;
  let parsed = SocketAddr::new(ip, port);
  if family == "ipv4" && parsed.is_ipv4()
    || family == "ipv6" && parsed.is_ipv6()
  {
    return Ok((ip, port, family));
  }

  bail!("Invalid address");
}

#[op2(fast)]
#[smi]
pub fn op_blocklist_new(state: &mut OpState) -> ResourceId {
  let blocklist = BlockList::new();
  let rid = state.resource_table.add(BlockListResource {
    blocklist: RefCell::new(blocklist),
  });
  rid
}

#[op2(fast)]
pub fn op_blocklist_add_address(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] address: String,
) -> Result<(), AnyError> {
  let wrap = state.resource_table.get::<BlockListResource>(rid).unwrap();
  let r = wrap.blocklist.borrow_mut().add_address(&address);
  r
}

#[op2(fast)]
pub fn op_blocklist_add_range(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] start: String,
  #[string] end: String,
) -> Result<bool, AnyError> {
  let wrap = state.resource_table.get::<BlockListResource>(rid).unwrap();
  let r = wrap.blocklist.borrow_mut().add_range(&start, &end);
  r
}

#[op2(fast)]
pub fn op_blocklist_add_subnet(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] addr: String,
  #[smi] prefix: u8,
) -> Result<(), AnyError> {
  let wrap = state.resource_table.get::<BlockListResource>(rid).unwrap();
  let r = wrap.blocklist.borrow_mut().add_subnet(&addr, prefix);
  r
}

#[op2(fast)]
pub fn op_blocklist_check(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] addr: String,
  #[string] r#type: String,
) -> Result<bool, AnyError> {
  let wrap = state.resource_table.get::<BlockListResource>(rid).unwrap();
  let r = wrap.blocklist.borrow().check(&addr, r#type);
  r
}

struct BlockList {
  rules: HashSet<IpNetwork>,
}

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

  pub fn check(&self, addr: &str, r#type: String) -> Result<bool, AnyError> {
    let addr: IpAddr = addr.parse()?;
    match r#type.to_lowercase().as_str() {
      "ipv4" => {
        if let IpAddr::V6(_) = addr {
          return Ok(false);
        }
      }
      "ipv6" => {
        if let IpAddr::V4(_) = addr {
          return Ok(false);
        }
      }
      _ => {
        bail!("Invalid type");
      }
    }
    Ok(self.rules.iter().any(|net| net.contains(addr)))
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
    assert!(block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());
    assert!(block_list
      .check("::ffff:c0a8:1", "ipv6".to_string())
      .unwrap());

    // Single IPv6 address
    let mut block_list = BlockList::new();
    block_list.add_address("2001:db8::1").unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6".to_string()).unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());
  }

  #[test]
  fn test_add_range() {
    // IPv4 range
    let mut block_list = BlockList::new();
    block_list.add_range("192.168.0.1", "192.168.0.3").unwrap();
    assert!(block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());
    assert!(block_list.check("192.168.0.2", "ipv4".to_string()).unwrap());
    assert!(block_list.check("192.168.0.3", "ipv4".to_string()).unwrap());
    assert!(block_list
      .check("::ffff:c0a8:1", "ipv6".to_string())
      .unwrap());

    // IPv6 range
    let mut block_list = BlockList::new();
    block_list.add_range("2001:db8::1", "2001:db8::3").unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6".to_string()).unwrap());
    assert!(block_list.check("2001:db8::2", "ipv6".to_string()).unwrap());
    assert!(block_list.check("2001:db8::3", "ipv6".to_string()).unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());
  }

  #[test]
  fn test_add_subnet() {
    // IPv4 subnet
    let mut block_list = BlockList::new();
    block_list.add_subnet("192.168.0.0", 24).unwrap();
    assert!(block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());
    assert!(block_list
      .check("192.168.0.255", "ipv4".to_string())
      .unwrap());
    assert!(block_list
      .check("::ffff:c0a8:0", "ipv6".to_string())
      .unwrap());

    // IPv6 subnet
    let mut block_list = BlockList::new();
    block_list.add_subnet("2001:db8::", 64).unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6".to_string()).unwrap());
    assert!(block_list
      .check("2001:db8::ffff", "ipv6".to_string())
      .unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());
  }

  #[test]
  fn test_check() {
    // Check IPv4 presence
    let mut block_list = BlockList::new();
    block_list.add_address("192.168.0.1").unwrap();
    assert!(block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());

    // Check IPv6 presence
    let mut block_list = BlockList::new();
    block_list.add_address("2001:db8::1").unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6".to_string()).unwrap());

    // Check IPv4 not present
    let block_list = BlockList::new();
    assert!(!block_list.check("192.168.0.1", "ipv4".to_string()).unwrap());

    // Check IPv6 not present
    let block_list = BlockList::new();
    assert!(!block_list.check("2001:db8::1", "ipv6".to_string()).unwrap());

    // Check invalid IP version
    let block_list = BlockList::new();
    assert!(!block_list.check("192.168.0.1", "ipv6".to_string()).unwrap());

    // Check invalid type
    let mut block_list = BlockList::new();
    block_list.add_address("192.168.0.1").unwrap();
    assert!(block_list
      .check("192.168.0.1", "invalid_type".to_string())
      .is_err());
  }
}
