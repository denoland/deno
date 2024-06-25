// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod io;
pub mod ops;
pub mod ops_tls;
#[cfg(unix)]
pub mod ops_unix;
pub mod raw;
pub mod resolve_addr;
mod tcp;

use deno_core::error::AnyError;
use deno_core::OpState;
use deno_permissions::host::split_host_port;
use deno_permissions::host::Host;
use deno_permissions::NetDescriptor;
use deno_tls::rustls::RootCertStore;
use deno_tls::RootCertStoreProvider;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

pub const UNSTABLE_FEATURE_NAME: &str = "net";

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NetPermissionHost {
  pub host: Host,
  pub port: Option<u16>,
}

impl NetPermissionHost {
  pub fn from_host_and_maybe_port(
    host: &str,
    port: Option<u16>,
  ) -> Result<Self, AnyError> {
    let lowercased = host.to_lowercase();
    let extracted_host = lowercased.as_str();
    let (host_str, port_) = split_host_port(extracted_host)?;
    let host =
      Host::from_host_and_origin_host(host_str.as_str(), extracted_host)?;

    let final_port = if let Some(port_) = port_ {
      Some(port_)
    } else {
      port
    };

    Ok(NetPermissionHost {
      host,
      port: final_port,
    })
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

impl NetPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_net(
    &mut self,
    host: &NetPermissionHost,
    api_name: &str,
  ) -> Result<(), AnyError> {
    let _host = host.clone().host;
    let _port = host.clone().port;
    deno_permissions::PermissionsContainer::check_net(
      self,
      &NetDescriptor(_host, _port),
      api_name,
    )
  }

  #[inline(always)]
  fn check_read(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_read(self, path, api_name)
  }

  #[inline(always)]
  fn check_write(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_write(self, path, api_name)
  }
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
  use deno_permissions::host::Host;
  use fqdn::FQDN;
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::str::FromStr;

  #[test]
  fn test_net_permission_host_parsing() {
    // Parsing host address without a port
    assert_eq!(
      NetPermissionHost::from_host_and_maybe_port("deno.land.", None).unwrap(),
      NetPermissionHost {
        host: Host::FQDN(FQDN::from_str("deno.land").unwrap()),
        port: None
      }
    );

    // Parsing host address with a port
    assert_eq!(
      NetPermissionHost::from_host_and_maybe_port("deno.land:80", None)
        .unwrap(),
      NetPermissionHost {
        host: Host::FQDN(FQDN::from_str("deno.land").unwrap()),
        port: Some(80)
      }
    );

    // Parsing an IPv4 address
    assert_eq!(
      NetPermissionHost::from_host_and_maybe_port("127.0.0.1", None).unwrap(),
      NetPermissionHost {
        host: Host::Ipv4(Ipv4Addr::new(127, 0, 0, 1)),
        port: None
      }
    );

    // Parsing an IPv4 address with a port
    assert_eq!(
      NetPermissionHost::from_host_and_maybe_port("127.0.0.1:80", None)
        .unwrap(),
      NetPermissionHost {
        host: Host::Ipv4(Ipv4Addr::new(127, 0, 0, 1)),
        port: Some(80)
      }
    );

    // Parsing an IPv6 address
    assert_eq!(
      NetPermissionHost::from_host_and_maybe_port(
        "[2606:4700:4700::1111]",
        None
      )
      .unwrap(),
      NetPermissionHost {
        host: Host::Ipv6(Ipv6Addr::new(
          0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111
        )),
        port: None
      }
    );

    // Parsing an IPv6 address with a port
    assert_eq!(
      NetPermissionHost::from_host_and_maybe_port(
        "[2606:4700:4700::1111]:80",
        None
      )
      .unwrap(),
      NetPermissionHost {
        host: Host::Ipv6(Ipv6Addr::new(
          0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111
        )),
        port: Some(80)
      }
    );

    // Parsing invalid host with special characters
    assert_eq!(
      NetPermissionHost::from_host_and_maybe_port("foo@bar.com.", None)
        .unwrap_err()
        .to_string(),
      "Failed to parse host: foo@bar.com.\n"
    );
  }
}
