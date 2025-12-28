// Copyright 2018-2025 the Deno authors. MIT license.

pub mod io;
pub mod ops;
pub mod ops_tls;
#[cfg(unix)]
pub mod ops_unix;
#[cfg(windows)]
mod ops_win_pipe;
mod quic;
pub mod raw;
pub mod resolve_addr;
pub mod tcp;
pub mod tunnel;
#[cfg(windows)]
mod win_pipe;

use std::sync::Arc;

use deno_core::OpState;
use deno_features::FeatureChecker;
use deno_tls::RootCertStoreProvider;
use deno_tls::rustls::RootCertStore;
pub use quic::QuicError;

pub const UNSTABLE_FEATURE_NAME: &str = "net";

/// Helper for checking unstable features. Used for sync ops.
fn check_unstable(state: &OpState, api_name: &str) {
  state
    .borrow::<Arc<FeatureChecker>>()
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
  ops = [
    ops::op_net_accept_tcp,
    ops::op_net_get_ips_from_perm_token,
    ops::op_net_connect_tcp,
    ops::op_net_listen_tcp,
    ops::op_net_listen_udp,
    ops::op_node_unstable_net_listen_udp,
    ops::op_net_recv_udp,
    ops::op_net_send_udp,
    ops::op_net_join_multi_v4_udp,
    ops::op_net_join_multi_v6_udp,
    ops::op_net_leave_multi_v4_udp,
    ops::op_net_leave_multi_v6_udp,
    ops::op_net_set_multi_loopback_udp,
    ops::op_net_set_multi_ttl_udp,
    ops::op_net_set_broadcast_udp,
    ops::op_net_validate_multicast,
    ops::op_dns_resolve,
    ops::op_set_nodelay,
    ops::op_set_keepalive,
    ops::op_net_listen_vsock,
    ops::op_net_accept_vsock,
    ops::op_net_connect_vsock,
    ops::op_net_listen_tunnel,
    ops::op_net_accept_tunnel,

    ops_tls::op_tls_key_null,
    ops_tls::op_tls_key_static,
    ops_tls::op_tls_cert_resolver_create,
    ops_tls::op_tls_cert_resolver_poll,
    ops_tls::op_tls_cert_resolver_resolve,
    ops_tls::op_tls_cert_resolver_resolve_error,
    ops_tls::op_tls_start,
    ops_tls::op_net_connect_tls,
    ops_tls::op_net_listen_tls,
    ops_tls::op_net_accept_tls,
    ops_tls::op_tls_handshake,

    ops_unix::op_net_accept_unix,
    ops_unix::op_net_connect_unix,
    ops_unix::op_net_listen_unix,
    ops_unix::op_net_listen_unixpacket,
    ops_unix::op_node_unstable_net_listen_unixpacket,
    ops_unix::op_net_recv_unixpacket,
    ops_unix::op_net_send_unixpacket,
    ops_unix::op_net_unix_stream_from_fd,

    ops_win_pipe::op_pipe_open,
    ops_win_pipe::op_pipe_connect,
    ops_win_pipe::op_pipe_windows_wait,

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
    quic::op_quic_endpoint_connect,
    quic::op_quic_endpoint_create,
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
  }

  stub_op!(op_net_accept_unix);
  stub_op!(op_net_connect_unix);
  stub_op!(op_net_listen_unix);
  stub_op!(op_net_listen_unixpacket);
  stub_op!(op_node_unstable_net_listen_unixpacket);
  stub_op!(op_net_recv_unixpacket);
  stub_op!(op_net_send_unixpacket);
  stub_op!(op_net_unix_stream_from_fd);
}

/// Stub ops for non-windows platforms.
#[cfg(not(windows))]
mod ops_win_pipe {
  use deno_core::op2;

  use crate::ops::NetError;

  #[op2(fast)]
  #[smi]
  pub fn op_pipe_open() -> Result<u32, NetError> {
    Err(NetError::Io(std::io::Error::new(
      std::io::ErrorKind::Unsupported,
      "Windows named pipes are not supported on this platform",
    )))
  }

  #[op2(fast)]
  #[smi]
  pub fn op_pipe_connect() -> Result<u32, NetError> {
    Err(NetError::Io(std::io::Error::new(
      std::io::ErrorKind::Unsupported,
      "Windows named pipes are not supported on this platform",
    )))
  }

  #[op2(fast)]
  pub fn op_pipe_windows_wait() -> Result<(), NetError> {
    Err(NetError::Io(std::io::Error::new(
      std::io::ErrorKind::Unsupported,
      "Windows named pipes are not supported on this platform",
    )))
  }
}
