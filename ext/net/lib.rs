// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub mod io;
pub mod ops;
pub mod ops_tls;
#[cfg(unix)]
pub mod ops_unix;
pub mod resolve_addr;

use deno_core::error::AnyError;
use deno_core::OpState;
use deno_tls::rustls::RootCertStore;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub trait NetPermissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    _host: &(T, Option<u16>),
    _api_name: &str,
  ) -> Result<(), AnyError>;
  fn check_read(&mut self, _p: &Path, _api_name: &str) -> Result<(), AnyError>;
  fn check_write(&mut self, _p: &Path, _api_name: &str)
    -> Result<(), AnyError>;
}

/// `UnstableChecker` is a struct so it can be placed inside `GothamState`;
/// using type alias for a bool could work, but there's a high chance
/// that there might be another type alias pointing to a bool, which
/// would override previously used alias.
pub struct UnstableChecker {
  pub unstable: bool,
}

impl UnstableChecker {
  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  // NOTE(bartlomieju): keep in sync with `cli/program_state.rs`
  pub fn check_unstable(&self, api_name: &str) {
    if !self.unstable {
      eprintln!(
        "Unstable API '{api_name}'. The --unstable flag must be provided."
      );
      std::process::exit(70);
    }
  }
}
/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, api_name: &str) {
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

/// Helper for checking unstable features. Used for async ops.
pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_net.d.ts")
}

#[derive(Clone)]
pub struct DefaultTlsOptions {
  pub root_cert_store: Option<RootCertStore>,
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
    ops::op_net_join_multi_v4_udp<P>,
    ops::op_net_join_multi_v6_udp<P>,
    ops::op_net_leave_multi_v4_udp<P>,
    ops::op_net_leave_multi_v6_udp<P>,
    ops::op_net_set_multi_loopback_udp<P>,
    ops::op_net_set_multi_ttl_udp<P>,
    ops::op_dns_resolve<P>,
    ops::op_set_nodelay,
    ops::op_set_keepalive,

    ops_tls::op_tls_start<P>,
    ops_tls::op_net_connect_tls<P>,
    ops_tls::op_net_listen_tls<P>,
    ops_tls::op_net_accept_tls,
    ops_tls::op_tls_handshake,

    #[cfg(unix)] ops_unix::op_net_accept_unix,
    #[cfg(unix)] ops_unix::op_net_connect_unix<P>,
    #[cfg(unix)] ops_unix::op_net_listen_unix<P>,
    #[cfg(unix)] ops_unix::op_net_listen_unixpacket<P>,
    #[cfg(unix)] ops_unix::op_node_unstable_net_listen_unixpacket<P>,
    #[cfg(unix)] ops_unix::op_net_recv_unixpacket,
    #[cfg(unix)] ops_unix::op_net_send_unixpacket<P>,
  ],
  esm = [ "01_net.js", "02_tls.js" ],
  options = {
    root_cert_store: Option<RootCertStore>,
    unstable: bool,
    unsafely_ignore_certificate_errors: Option<Vec<String>>,
  },
  state = |state, options| {
    state.put(DefaultTlsOptions {
      root_cert_store: options.root_cert_store,
    });
    state.put(UnstableChecker { unstable: options.unstable });
    state.put(UnsafelyIgnoreCertificateErrors(
      options.unsafely_ignore_certificate_errors,
    ));
  },
);
