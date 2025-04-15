// Copyright 2018-2025 the Deno authors. MIT license.

pub mod io;
pub mod ops;
pub mod ops_tls;
#[cfg(unix)]
pub mod ops_unix;
mod quic;
pub mod raw;
pub mod resolve_addr;
pub mod tcp;

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::OpState;
use deno_permissions::PermissionCheckError;
use deno_tls::rustls::RootCertStore;
use deno_tls::RootCertStoreProvider;
pub use quic::QuicError;

pub const UNSTABLE_FEATURE_NAME: &str = "net";

pub trait NetPermissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    host: &(T, Option<u16>),
    api_name: &str,
  ) -> Result<(), PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_read(
    &mut self,
    p: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_write(
    &mut self,
    p: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_write_path<'a>(
    &mut self,
    p: &'a Path,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_vsock(
    &mut self,
    cid: u32,
    port: u32,
    api_name: &str,
  ) -> Result<(), PermissionCheckError>;
}

impl NetPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_net<T: AsRef<str>>(
    &mut self,
    host: &(T, Option<u16>),
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_net(self, host, api_name)
  }

  #[inline(always)]
  fn check_read(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_read(self, path, api_name)
  }

  #[inline(always)]
  fn check_write(
    &mut self,
    path: &str,
    api_name: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write(self, path, api_name)
  }

  #[inline(always)]
  fn check_write_path<'a>(
    &mut self,
    path: &'a Path,
    api_name: &str,
  ) -> Result<Cow<'a, Path>, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_write_path(
      self, path, api_name,
    )
  }

  #[inline(always)]
  fn check_vsock(
    &mut self,
    cid: u32,
    port: u32,
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_net_vsock(
      self, cid, port, api_name,
    )
  }
}

/// Helper for checking unstable features. Used for sync ops.
fn check_unstable(state: &OpState, api_name: &str) {
  state
    .feature_checker
    .check_or_exit(UNSTABLE_FEATURE_NAME, api_name);
}

#[derive(Clone)]
pub struct DefaultTlsOptions {
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
}

impl DefaultTlsOptions {
  pub fn root_cert_store(
    &self,
  ) -> Result<Option<RootCertStore>, deno_error::JsErrorBox> {
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
    ops::op_net_get_ips_from_perm_token,
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
    ops::op_net_listen_vsock<P>,
    ops::op_net_accept_vsock,
    ops::op_net_connect_vsock<P>,

    ops_tls::op_tls_key_null,
    ops_tls::op_tls_key_static,
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

    quic::op_quic_connecting_0rtt,
    quic::op_quic_connecting_1rtt,
    quic::op_quic_connection_accept_bi,
    quic::op_quic_connection_accept_uni,
    quic::op_quic_connection_close,
    quic::op_quic_connection_closed,
    quic::op_quic_connection_get_protocol,
    quic::op_quic_connection_get_remote_addr,
    quic::op_quic_connection_get_server_name,
    quic::op_quic_connection_handshake,
    quic::op_quic_connection_open_bi,
    quic::op_quic_connection_open_uni,
    quic::op_quic_connection_get_max_datagram_size,
    quic::op_quic_connection_read_datagram,
    quic::op_quic_connection_send_datagram,
    quic::op_quic_endpoint_close,
    quic::op_quic_endpoint_connect<P>,
    quic::op_quic_endpoint_create<P>,
    quic::op_quic_endpoint_get_addr,
    quic::op_quic_endpoint_listen,
    quic::op_quic_incoming_accept,
    quic::op_quic_incoming_accept_0rtt,
    quic::op_quic_incoming_ignore,
    quic::op_quic_incoming_local_ip,
    quic::op_quic_incoming_refuse,
    quic::op_quic_incoming_remote_addr,
    quic::op_quic_incoming_remote_addr_validated,
    quic::op_quic_listener_accept,
    quic::op_quic_listener_stop,
    quic::op_quic_recv_stream_get_id,
    quic::op_quic_send_stream_get_id,
    quic::op_quic_send_stream_get_priority,
    quic::op_quic_send_stream_set_priority,
    quic::webtransport::op_webtransport_accept,
    quic::webtransport::op_webtransport_connect,
  ],
  esm = [ "01_net.js", "02_tls.js" ],
  lazy_loaded_esm = [ "03_quic.js" ],
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
  use deno_core::op2;

  use crate::NetPermissions;

  macro_rules! stub_op {
    ($name:ident) => {
      #[op2(fast)]
      pub fn $name() -> Result<(), std::io::Error> {
        let error_msg = format!(
          "Operation `{:?}` not supported on non-unix platforms.",
          stringify!($name)
        );
        Err(std::io::Error::new(
          std::io::ErrorKind::Unsupported,
          error_msg,
        ))
      }
    };
    ($name:ident<P>) => {
      #[op2(fast)]
      pub fn $name<P: NetPermissions>() -> Result<(), std::io::Error> {
        let error_msg = format!(
          "Operation `{:?}` not supported on non-unix platforms.",
          stringify!($name)
        );
        Err(std::io::Error::new(
          std::io::ErrorKind::Unsupported,
          error_msg,
        ))
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
