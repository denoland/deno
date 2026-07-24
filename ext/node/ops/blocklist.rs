// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashSet;
use std::net::IpAddr;
use std::net::SocketAddr;

use deno_core::OpState;
use deno_core::ToV8;
use deno_core::op2;
use ipnet::IpNet;
use ipnet::Ipv4Net;
use ipnet::Ipv6Net;

pub struct BlockListResource {
  blocklist: RefCell<BlockList>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for BlockListResource {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"BlockListResource"
  }
}

#[derive(ToV8)]
struct SocketAddressSerialization(String, String);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
pub enum BlocklistError {
  #[error("{0}")]
  AddrParse(#[from] std::net::AddrParseError),
  #[error("{0}")]
  PrefixLen(#[from] ipnet::PrefixLenError),
  #[error("Invalid address")]
  InvalidAddress,
  #[error("IP version mismatch between start and end addresses")]
  IpVersionMismatch,
}

#[op2(fast)]
pub fn op_socket_address_parse(
  state: &mut OpState,
  #[string] addr: &str,
  #[smi] port: u16,
  #[string] family: &str,
) -> Result<bool, BlocklistError> {
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
    Err(BlocklistError::InvalidAddress)
  }
}

#[op2]
pub fn op_socket_address_get_serialization(
  state: &mut OpState,
) -> SocketAddressSerialization {
  state.take::<SocketAddressSerialization>()
}

#[op2]
#[cppgc]
pub fn op_blocklist_new() -> BlockListResource {
  let blocklist = BlockList::new();
  BlockListResource {
    blocklist: RefCell::new(blocklist),
  }
}

#[op2(fast)]
pub fn op_blocklist_add_address(
  #[cppgc] wrap: &BlockListResource,
  #[string] addr: &str,
) -> Result<(), BlocklistError> {
  wrap.blocklist.borrow_mut().add_address(addr)
}

#[op2(fast)]
pub fn op_blocklist_add_range(
  #[cppgc] wrap: &BlockListResource,
  #[string] start: &str,
  #[string] end: &str,
) -> Result<bool, BlocklistError> {
  wrap.blocklist.borrow_mut().add_range(start, end)
}

#[op2(fast)]
pub fn op_blocklist_add_subnet(
  #[cppgc] wrap: &BlockListResource,
  #[string] addr: &str,
  #[smi] prefix: u8,
) -> Result<(), BlocklistError> {
  wrap.blocklist.borrow_mut().add_subnet(addr, prefix)
}

#[op2(fast)]
pub fn op_blocklist_check(
  #[cppgc] wrap: &BlockListResource,
  #[string] addr: &str,
  #[string] r#type: &str,
) -> Result<bool, BlocklistError> {
  wrap.blocklist.borrow().check(addr, r#type)
}

struct BlockList {
  rules: HashSet<IpNet>,
  ranges: Vec<IpRange>,
}

#[derive(Debug, Clone, Copy)]
enum IpRange {
  V4 { start: u32, end: u32 },
  V6 { start: u128, end: u128 },
}

impl IpRange {
  fn contains(&self, addr: IpAddr) -> bool {
    match (self, addr) {
      (IpRange::V4 { start, end }, IpAddr::V4(addr)) => {
        let addr = u32::from(addr);
        *start <= addr && addr <= *end
      }
      (IpRange::V6 { start, end }, IpAddr::V6(addr)) => {
        let addr = u128::from(addr);
        *start <= addr && addr <= *end
      }
      _ => false,
    }
  }
}

const IPV4_MAPPED_V6_START: u128 = 0xffff_u128 << 32;
const IPV4_MAPPED_V6_END: u128 = IPV4_MAPPED_V6_START | u32::MAX as u128;

fn ipv4_mapped_v6_addr(addr: u32) -> u128 {
  IPV4_MAPPED_V6_START | u128::from(addr)
}

fn ipv4_mapped_v6_range_to_v4(start: u128, end: u128) -> Option<(u32, u32)> {
  let start = start.max(IPV4_MAPPED_V6_START);
  let end = end.min(IPV4_MAPPED_V6_END);
  if end < start {
    return None;
  }
  Some((
    (start - IPV4_MAPPED_V6_START) as u32,
    (end - IPV4_MAPPED_V6_START) as u32,
  ))
}

impl BlockList {
  pub fn new() -> Self {
    BlockList {
      rules: HashSet::new(),
      ranges: Vec::new(),
    }
  }

  fn map_addr_add_network(
    &mut self,
    addr: IpAddr,
    prefix: Option<u8>,
  ) -> Result<(), BlocklistError> {
    match addr {
      IpAddr::V4(addr) => {
        let ipv4_prefix = prefix.unwrap_or(32);
        self
          .rules
          .insert(IpNet::V4(Ipv4Net::new(addr, ipv4_prefix)?));

        let ipv6_mapped = addr.to_ipv6_mapped();
        let ipv6_prefix = 96 + ipv4_prefix; // IPv4-mapped IPv6 address prefix starts at 96
        self
          .rules
          .insert(IpNet::V6(Ipv6Net::new(ipv6_mapped, ipv6_prefix)?));
      }
      IpAddr::V6(addr) => {
        if let Some(ipv4_mapped) = addr.to_ipv4_mapped() {
          let ipv4_prefix = prefix.map(|v| v.clamp(96, 128) - 96).unwrap_or(32);
          self
            .rules
            .insert(IpNet::V4(Ipv4Net::new(ipv4_mapped, ipv4_prefix)?));
        }

        let ipv6_prefix = prefix.unwrap_or(128);
        self
          .rules
          .insert(IpNet::V6(Ipv6Net::new(addr, ipv6_prefix)?));
      }
    };
    Ok(())
  }

  pub fn add_address(&mut self, address: &str) -> Result<(), BlocklistError> {
    let ip: IpAddr = address.parse()?;
    self.map_addr_add_network(ip, None)?;
    Ok(())
  }

  pub fn add_range(
    &mut self,
    start: &str,
    end: &str,
  ) -> Result<bool, BlocklistError> {
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
        self.ranges.push(IpRange::V4 {
          start: start_u32,
          end: end_u32,
        });
        self.ranges.push(IpRange::V6 {
          start: ipv4_mapped_v6_addr(start_u32),
          end: ipv4_mapped_v6_addr(end_u32),
        });
      }
      (IpAddr::V6(start), IpAddr::V6(end)) => {
        let start_u128: u128 = start.into();
        let end_u128: u128 = end.into();
        if end_u128 < start_u128 {
          // Indicates invalid range.
          return Ok(false);
        }
        self.ranges.push(IpRange::V6 {
          start: start_u128,
          end: end_u128,
        });
        if let Some((start, end)) =
          ipv4_mapped_v6_range_to_v4(start_u128, end_u128)
        {
          self.ranges.push(IpRange::V4 { start, end });
        }
      }
      _ => return Err(BlocklistError::IpVersionMismatch),
    }
    Ok(true)
  }

  pub fn add_subnet(
    &mut self,
    addr: &str,
    prefix: u8,
  ) -> Result<(), BlocklistError> {
    let ip: IpAddr = addr.parse()?;
    self.map_addr_add_network(ip, Some(prefix))?;
    Ok(())
  }

  pub fn check(
    &self,
    addr: &str,
    r#type: &str,
  ) -> Result<bool, BlocklistError> {
    let addr: IpAddr = addr.parse()?;
    let family = r#type.to_lowercase();
    if family == "ipv4" && addr.is_ipv4() || family == "ipv6" && addr.is_ipv6()
    {
      Ok(
        self.rules.iter().any(|net| net.contains(&addr))
          || self.ranges.iter().any(|range| range.contains(addr)),
      )
    } else {
      Err(BlocklistError::InvalidAddress)
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
    assert!(block_list.rules.is_empty());
    assert_eq!(block_list.ranges.len(), 2);
    assert!(block_list.check("192.168.0.1", "ipv4").unwrap());
    assert!(block_list.check("192.168.0.2", "ipv4").unwrap());
    assert!(block_list.check("192.168.0.3", "ipv4").unwrap());
    assert!(block_list.check("::ffff:c0a8:1", "ipv6").unwrap());

    // IPv6 range
    let mut block_list = BlockList::new();
    block_list.add_range("2001:db8::1", "2001:db8::3").unwrap();
    assert!(block_list.rules.is_empty());
    assert_eq!(block_list.ranges.len(), 1);
    assert!(block_list.check("2001:db8::1", "ipv6").unwrap());
    assert!(block_list.check("2001:db8::2", "ipv6").unwrap());
    assert!(block_list.check("2001:db8::3", "ipv6").unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4").unwrap());
  }

  #[test]
  fn test_add_large_range() {
    // Ranges must be stored as ranges rather than expanded into each address.
    let mut block_list = BlockList::new();
    block_list.add_range("0.0.0.0", "255.255.255.255").unwrap();
    assert!(block_list.check("1.2.3.4", "ipv4").unwrap());
    assert!(block_list.check("::ffff:1.2.3.4", "ipv6").unwrap());

    let mut block_list = BlockList::new();
    block_list
      .add_range("::", "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff")
      .unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6").unwrap());
    assert!(block_list.check("::ffff:10.0.0.1", "ipv6").unwrap());
    assert!(block_list.check("10.0.0.1", "ipv4").unwrap());
  }

  #[test]
  fn test_add_ipv4_mapped_ipv6_range() {
    let mut block_list = BlockList::new();
    block_list
      .add_range("::ffff:10.0.0.2", "::ffff:10.0.0.10")
      .unwrap();
    assert!(block_list.check("::ffff:10.0.0.5", "ipv6").unwrap());
    assert!(block_list.check("10.0.0.5", "ipv4").unwrap());
    assert!(!block_list.check("10.0.0.11", "ipv4").unwrap());
  }

  #[test]
  fn test_ipv4_mapped_ipv6_range_intersection() {
    assert_eq!(
      ipv4_mapped_v6_range_to_v4(
        IPV4_MAPPED_V6_START - 1,
        IPV4_MAPPED_V6_START,
      ),
      Some((0, 0)),
    );
    assert_eq!(
      ipv4_mapped_v6_range_to_v4(IPV4_MAPPED_V6_END, IPV4_MAPPED_V6_END + 1,),
      Some((u32::MAX, u32::MAX)),
    );
    assert_eq!(
      ipv4_mapped_v6_range_to_v4(0, IPV4_MAPPED_V6_START - 1),
      None,
    );
    assert_eq!(
      ipv4_mapped_v6_range_to_v4(IPV4_MAPPED_V6_END + 1, u128::MAX),
      None,
    );
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
    block_list.add_subnet("::ffff:127.0.0.1", 128).unwrap();
    assert!(block_list.check("2001:db8::1", "ipv6").unwrap());
    assert!(block_list.check("2001:db8::ffff", "ipv6").unwrap());
    assert!(!block_list.check("192.168.0.1", "ipv4").unwrap());

    // Check host addresses of IPv4 mapped IPv6 address
    let mut block_list = BlockList::new();
    block_list.add_subnet("1.1.1.0", 30).unwrap();
    assert!(block_list.check("::ffff:1.1.1.1", "ipv6").unwrap());
    assert!(!block_list.check("::ffff:1.1.1.4", "ipv6").unwrap());
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
