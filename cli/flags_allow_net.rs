// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::url::Url;
use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq)]
pub struct ParsePortError(String);

#[derive(Debug, PartialEq, Eq)]
pub struct BarePort(u16);

impl FromStr for BarePort {
  type Err = ParsePortError;
  fn from_str(s: &str) -> Result<BarePort, ParsePortError> {
    if s.starts_with(':') {
      match s.split_at(1).1.parse::<u16>() {
        Ok(port) => Ok(BarePort(port)),
        Err(e) => Err(ParsePortError(e.to_string())),
      }
    } else {
      Err(ParsePortError(
        "Bare Port doesn't start with ':'".to_string(),
      ))
    }
  }
}

pub fn validator(host_and_port: String) -> Result<(), String> {
  if Url::parse(&format!("deno://{}", host_and_port)).is_ok()
    || host_and_port.parse::<IpAddr>().is_ok()
    || host_and_port.parse::<BarePort>().is_ok()
  {
    Ok(())
  } else {
    Err(format!("Bad host:port pair: {}", host_and_port))
  }
}

/// Expands "bare port" paths (eg. ":8080") into full paths with hosts. It
/// expands to such paths into 3 paths with following hosts: `0.0.0.0:port`,
/// `127.0.0.1:port` and `localhost:port`.
pub fn parse(paths: Vec<String>) -> clap::Result<Vec<String>> {
  let mut out: Vec<String> = vec![];
  for host_and_port in paths.iter() {
    if Url::parse(&format!("deno://{}", host_and_port)).is_ok()
      || host_and_port.parse::<IpAddr>().is_ok()
    {
      out.push(host_and_port.to_owned())
    } else if let Ok(port) = host_and_port.parse::<BarePort>() {
      // we got bare port, let's add default hosts
      for host in ["0.0.0.0", "127.0.0.1", "localhost"].iter() {
        out.push(format!("{}:{}", host, port.0));
      }
    } else {
      return Err(clap::Error::with_description(
        &format!("Bad host:port pair: {}", host_and_port),
        clap::ErrorKind::InvalidValue,
      ));
    }
  }
  Ok(out)
}

#[cfg(test)]
mod bare_port_tests {
  use super::{BarePort, ParsePortError};

  #[test]
  fn bare_port_parsed() {
    let expected = BarePort(8080);
    let actual = ":8080".parse::<BarePort>();
    assert_eq!(actual, Ok(expected));
  }

  #[test]
  fn bare_port_parse_error1() {
    let expected =
      ParsePortError("Bare Port doesn't start with ':'".to_string());
    let actual = "8080".parse::<BarePort>();
    assert_eq!(actual, Err(expected));
  }

  #[test]
  fn bare_port_parse_error2() {
    let actual = ":65536".parse::<BarePort>();
    assert!(actual.is_err());
  }

  #[test]
  fn bare_port_parse_error3() {
    let actual = ":14u16".parse::<BarePort>();
    assert!(actual.is_err());
  }

  #[test]
  fn bare_port_parse_error4() {
    let actual = "Deno".parse::<BarePort>();
    assert!(actual.is_err());
  }

  #[test]
  fn bare_port_parse_error5() {
    let actual = "deno.land:8080".parse::<BarePort>();
    assert!(actual.is_err());
  }
}

#[cfg(test)]
mod tests {
  use super::parse;

  // Creates vector of strings, Vec<String>
  macro_rules! svec {
      ($($x:expr),*) => (vec![$($x.to_string()),*]);
  }

  #[test]
  fn parse_net_args_() {
    let entries = svec![
      "deno.land",
      "deno.land:80",
      "::",
      "::1",
      "127.0.0.1",
      "[::1]",
      "1.2.3.4:5678",
      "0.0.0.0:5678",
      "127.0.0.1:5678",
      "[::]:5678",
      "[::1]:5678",
      "localhost:5678",
      "[::1]:8080",
      "[::]:8000",
      "[::1]:8000",
      "localhost:8000",
      "0.0.0.0:4545",
      "127.0.0.1:4545",
      "999.0.88.1:80"
    ];
    let expected = svec![
      "deno.land",
      "deno.land:80",
      "::",
      "::1",
      "127.0.0.1",
      "[::1]",
      "1.2.3.4:5678",
      "0.0.0.0:5678",
      "127.0.0.1:5678",
      "[::]:5678",
      "[::1]:5678",
      "localhost:5678",
      "[::1]:8080",
      "[::]:8000",
      "[::1]:8000",
      "localhost:8000",
      "0.0.0.0:4545",
      "127.0.0.1:4545",
      "999.0.88.1:80"
    ];
    let actual = parse(entries).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn parse_net_args_expansion() {
    let entries = svec![":8080"];
    let expected = svec!["0.0.0.0:8080", "127.0.0.1:8080", "localhost:8080"];
    let actual = parse(entries).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn parse_net_args_ipv6() {
    let entries =
      svec!["::", "::1", "[::1]", "[::]:5678", "[::1]:5678", "::cafe"];
    let expected =
      svec!["::", "::1", "[::1]", "[::]:5678", "[::1]:5678", "::cafe"];
    let actual = parse(entries).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn parse_net_args_ipv6_error1() {
    let entries = svec![":::"];
    assert!(parse(entries).is_err());
  }

  #[test]
  fn parse_net_args_ipv6_error2() {
    let entries = svec!["0123:4567:890a:bcde:fg::"];
    assert!(parse(entries).is_err());
  }

  #[test]
  fn parse_net_args_ipv6_error3() {
    let entries = svec!["[::q]:8080"];
    assert!(parse(entries).is_err());
  }
}
