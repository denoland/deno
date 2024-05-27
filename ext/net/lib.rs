// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod io;
pub mod ops;
pub mod ops_tls;
#[cfg(unix)]
pub mod ops_unix;
pub mod raw;
pub mod resolve_addr;
mod tcp;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_tls::rustls::RootCertStore;
use deno_tls::RootCertStoreProvider;
use fqdn::FQDN;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

pub const UNSTABLE_FEATURE_NAME: &str = "net";

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NetPermissionHost {
  pub host: String,
  pub port: Option<u16>,
}

impl NetPermissionHost {
  pub fn from_str(host: &str, mut port: Option<u16>) -> Result<Self, AnyError> {
    // Extract the host portion from a potential URL format (e.g., https://host:port)
    let mut extracted_host = Self::extract_host(host);

    // Handle IPv6 addresses
    if extracted_host.starts_with('[') {
      if extracted_host.ends_with("]:") {
        return Err(AnyError::msg("Invalid format: [ipv6]:port"));
      }
      if let Some(pos) = extracted_host.rfind("]:") {
        let port_str = &extracted_host[pos + 2..];
        let port_ = port_str.parse::<u16>().ok();
        extracted_host = extracted_host[1..pos].to_string();
        return Self::handle_ipv6(extracted_host, port_);
      } else {
        extracted_host =
          extracted_host[1..(extracted_host.len() - 1)].to_string();
        return Self::handle_ipv6(extracted_host, port);
      }
    }

    // Handle IPv4 addresses and hostnames with optional ports
    if let Some((host, port_)) = Self::split_host_port(&extracted_host) {
      if port_.is_some() {
        port = port_;
      }
      let fqdn = FQDN::from_str(&host).with_context(|| {
        format!("Failed to parse host: {}\n", &extracted_host)
      })?;
      let host_str = fqdn.to_string();
      if host_str.parse::<Ipv4Addr>().is_ok() {
        return Ok(NetPermissionHost {
          host: host_str,
          port,
        });
      }
      Ok(NetPermissionHost {
        host: host_str,
        port,
      })
    } else {
      Err(AnyError::msg("Failed to parse input string"))
    }
  }

  fn extract_host(s: &str) -> String {
    let mut extracted_host = s.to_string();
    if let Some(index) = extracted_host.find("://") {
      extracted_host = extracted_host[index + 3..]
        .split('/')
        .next()
        .unwrap_or(&extracted_host)
        .to_string();
    }
    extracted_host
  }

  fn handle_ipv6(
    host: String,
    port: Option<u16>,
  ) -> Result<NetPermissionHost, AnyError> {
    Ok(NetPermissionHost {
      host: format!("[{}]", host.parse::<Ipv6Addr>()?.to_string()),
      port,
    })
  }

  fn split_host_port(s: &str) -> Option<(String, Option<u16>)> {
    if let Some(pos) = s.rfind(':') {
      let port_str = &s[pos + 1..];
      if let Ok(parsed_port) = port_str.parse::<u16>() {
        let host = s[0..pos].to_string();
        return Some((host, Some(parsed_port)));
      }
    }
    Some((s.to_string(), None))
  }
}

pub trait NetPermissions {
  fn check_net(
    &mut self,
    _host: &NetPermissionHost,
    _api_name: &str,
  ) -> Result<(), AnyError>;
  fn check_read(&mut self, _p: &Path, _api_name: &str) -> Result<(), AnyError>;
  fn check_write(&mut self, _p: &Path, _api_name: &str)
    -> Result<(), AnyError>;
}

/// Helper for checking unstable features. Used for sync ops.
fn check_unstable(state: &OpState, api_name: &str) {
  // TODO(bartlomieju): replace with `state.feature_checker.check_or_exit`
  // once we phase out `check_or_exit_with_legacy_fallback`
  state
    .feature_checker
    .check_or_exit_with_legacy_fallback(UNSTABLE_FEATURE_NAME, api_name);
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_net.d.ts")
}

#[derive(Clone)]
pub struct DefaultTlsOptions {
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
}

impl DefaultTlsOptions {
  pub fn root_cert_store(&self) -> Result<Option<RootCertStore>, AnyError> {
    Ok(match &self.root_cert_store_provider {
      Some(provider) => Some(provider.get_or_try_init()?.clone()),
      None => None,
    })
  }
}

/// `UnsafelyIgnoreCertificateErrors` is a wrapper struct so it can be placed inside `GothamState`;
/// using type alias for a `Option<Vec<String>>` could work, but there's a high chance
/// that there might be another type alias pointing to a `Option<Vec<String>>`, which
/// would override previously used alias.
pub struct UnsafelyIgnoreCertificateErrors(pub Option<Vec<String>>);

deno_core::extension!(deno_net,
  deps = [ deno_web ],
  parameters = [ P: NetPermissions ],
  ops = [
    ops::op_net_accept_tcp,
    ops::op_net_connect_tcp<P>,
    ops::op_net_listen_tcp<P>,
    ops::op_net_listen_udp<P>,
    ops::op_node_unstable_net_listen_udp<P>,
    ops::op_net_recv_udp,
    ops::op_net_send_udp<P>,
    ops::op_net_join_multi_v4_udp,
    ops::op_net_join_multi_v6_udp,
    ops::op_net_leave_multi_v4_udp,
    ops::op_net_leave_multi_v6_udp,
    ops::op_net_set_multi_loopback_udp,
    ops::op_net_set_multi_ttl_udp,
    ops::op_dns_resolve<P>,
    ops::op_set_nodelay,
    ops::op_set_keepalive,

    ops_tls::op_tls_key_null,
    ops_tls::op_tls_key_static,
    ops_tls::op_tls_key_static_from_file<P>,
    ops_tls::op_tls_cert_resolver_create,
    ops_tls::op_tls_cert_resolver_poll,
    ops_tls::op_tls_cert_resolver_resolve,
    ops_tls::op_tls_cert_resolver_resolve_error,
    ops_tls::op_tls_start<P>,
    ops_tls::op_net_connect_tls<P>,
    ops_tls::op_net_listen_tls<P>,
    ops_tls::op_net_accept_tls,
    ops_tls::op_tls_handshake,

    ops_unix::op_net_accept_unix,
    ops_unix::op_net_connect_unix<P>,
    ops_unix::op_net_listen_unix<P>,
    ops_unix::op_net_listen_unixpacket<P>,
    ops_unix::op_node_unstable_net_listen_unixpacket<P>,
    ops_unix::op_net_recv_unixpacket,
    ops_unix::op_net_send_unixpacket<P>,
  ],
  esm = [ "01_net.js", "02_tls.js" ],
  options = {
    root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
    unsafely_ignore_certificate_errors: Option<Vec<String>>,
  },
  state = |state, options| {
    state.put(DefaultTlsOptions {
      root_cert_store_provider: options.root_cert_store_provider,
    });
    state.put(UnsafelyIgnoreCertificateErrors(
      options.unsafely_ignore_certificate_errors,
    ));
  },
);

/// Stub ops for non-unix platforms.
#[cfg(not(unix))]
mod ops_unix {
  use crate::NetPermissions;
  use deno_core::op2;

  macro_rules! stub_op {
    ($name:ident) => {
      #[op2(fast)]
      pub fn $name() {
        panic!("Unsupported on non-unix platforms")
      }
    };
    ($name:ident<P>) => {
      #[op2(fast)]
      pub fn $name<P: NetPermissions>() {
        panic!("Unsupported on non-unix platforms")
      }
    };
  }

  stub_op!(op_net_accept_unix);
  stub_op!(op_net_connect_unix<P>);
  stub_op!(op_net_listen_unix<P>);
  stub_op!(op_net_listen_unixpacket<P>);
  stub_op!(op_node_unstable_net_listen_unixpacket<P>);
  stub_op!(op_net_recv_unixpacket);
  stub_op!(op_net_send_unixpacket<P>);
}

#[cfg(test)]
mod tests {
  use super::NetPermissionHost;

  #[test]
  fn test_net_permission_host_parsing() {
    // Parsing host address without a port
    assert_eq!(
      NetPermissionHost::from_str("deno.land.", None).unwrap(),
      NetPermissionHost {
        host: "deno.land".to_string(),
        port: None
      }
    );
    // Parsing host address with a port
    assert_eq!(
      NetPermissionHost::from_str("deno.land:80", None).unwrap(),
      NetPermissionHost {
        host: "deno.land".to_string(),
        port: Some(80)
      }
    );

    // Parsing an IPv4 address
    assert_eq!(
      NetPermissionHost::from_str("127.0.0.1", None).unwrap(),
      NetPermissionHost {
        host: "127.0.0.1".to_string(),
        port: None
      }
    );
    // Parsing an IPv4 address with a port
    assert_eq!(
      NetPermissionHost::from_str("127.0.0.1:80", None).unwrap(),
      NetPermissionHost {
        host: "127.0.0.1".to_string(),
        port: Some(80)
      }
    );

    // Parsing an IPv6 address
    assert_eq!(
      NetPermissionHost::from_str("[2606:4700:4700::1111]", None).unwrap(),
      NetPermissionHost {
        host: "[2606:4700:4700::1111]".to_string(),
        port: None
      }
    );
    // Parsing an IPv6 address with a port
    assert_eq!(
      NetPermissionHost::from_str("[2606:4700:4700::1111]:80", None).unwrap(),
      NetPermissionHost {
        host: "[2606:4700:4700::1111]".to_string(),
        port: Some(80)
      }
    );

    // Parsing a URL with a host
    assert_eq!(
      NetPermissionHost::from_str("https://github.com/denoland/", None)
        .unwrap(),
      NetPermissionHost {
        host: "github.com".to_string(),
        port: None
      }
    );
    // Parsing a URL with a host & port
    assert_eq!(
      NetPermissionHost::from_str("https://github.com:443/denoland/", None)
        .unwrap(),
      NetPermissionHost {
        host: "github.com".to_string(),
        port: Some(443)
      }
    );

    // Parsing a URL with an IPv4 address
    assert_eq!(
      NetPermissionHost::from_str("https://127.0.0.1", None).unwrap(),
      NetPermissionHost {
        host: "127.0.0.1".to_string(),
        port: None
      }
    );
    // Parsing a URL with an IPv4 address & port
    assert_eq!(
      NetPermissionHost::from_str("https://127.0.0.1:80", None).unwrap(),
      NetPermissionHost {
        host: "127.0.0.1".to_string(),
        port: Some(80)
      }
    );

    // Parsing a URL with an IPv6 address
    assert_eq!(
      NetPermissionHost::from_str("https://[2606:4700:4700::1111]", None)
        .unwrap(),
      NetPermissionHost {
        host: "[2606:4700:4700::1111]".to_string(),
        port: None
      }
    );
    // Parsing a URL with an IPv6 address & port
    assert_eq!(
      NetPermissionHost::from_str("https://[2606:4700:4700::1111]:80", None)
        .unwrap(),
      NetPermissionHost {
        host: "[2606:4700:4700::1111]".to_string(),
        port: Some(80)
      }
    );

    // Parsing invalid URL/host with special characters
    assert_eq!(
      NetPermissionHost::from_str("foo@bar.com.", None)
        .unwrap_err()
        .to_string(),
      "Failed to parse host: foo@bar.com.\n"
    );
    assert_eq!(
      NetPermissionHost::from_str("http://foo@bar.com.:80", None)
        .unwrap_err()
        .to_string(),
      "Failed to parse host: foo@bar.com.:80\n"
    );
  }
}
