// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::uri_error;
use deno_core::error::AnyError;
use fqdn::FQDN;
use std::fmt;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::str::FromStr;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Host {
  FQDN(FQDN),
  Ipv4(Ipv4Addr),
  Ipv6(Ipv6Addr),
}

impl Host {
  pub fn from_host_and_origin_host(
    host: &str,
    origin_host: &str,
  ) -> Result<Self, AnyError> {
    if let Ok(ipv6) = host.parse::<Ipv6Addr>() {
      return Ok(Host::Ipv6(ipv6));
    }

    let host = FQDN::from_str(host).map_err(|_| {
      uri_error(format!("Failed to parse host: {}\n", origin_host))
    })?;
    let host_string = host.to_string();

    if let Ok(ipv4) = host_string.parse::<Ipv4Addr>() {
      return Ok(Host::Ipv4(ipv4));
    }

    Ok(Host::FQDN(host))
  }
}

impl fmt::Display for Host {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Host::FQDN(fqdn) => write!(f, "{}", fqdn),
      Host::Ipv4(ipv4) => write!(f, "{}", ipv4),
      Host::Ipv6(ipv6) => write!(f, "[{}]", ipv6),
    }
  }
}

pub fn split_host_port(s: &str) -> Result<(String, Option<u16>), AnyError> {
  let mut host = s.to_string();
  let mut port = None;

  let have_port = host.contains(':') && !host.contains('[');

  if host.starts_with('[') && host.contains(']') {
    if host.ends_with("]:") {
      return Err(uri_error("Invalid format: [ipv6]:port"));
    }
    if let Some(pos) = host.rfind("]:") {
      let port_str = &host[pos + 2..];
      let port_ = port_str.parse::<u16>().ok();
      host = host[1..pos].to_string();
      port = port_;
    } else {
      host = host[1..(host.len() - 1)].to_string();
    }
  } else if let Some(pos) = host.rfind(':') {
    let port_str = &host[pos + 1..];
    if let Ok(parsed_port) = port_str.parse::<u16>() {
      host.truncate(pos);
      port = Some(parsed_port);
    }
  }

  if have_port && port.is_none() {
    return Err(uri_error("No port specified after ':'"));
  }

  Ok((host, port))
}
