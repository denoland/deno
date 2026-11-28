// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::print_stdout, reason = "test server output")]
#![allow(clippy::print_stderr, reason = "test server output")]

pub use test_util::*;

pub mod https;
pub mod npm;
pub mod servers;

fn main() {
  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();
  setup_panic_hook();
  servers::run_all_servers();
}

/// Binds a TCP listener with a larger accept backlog than tokio's default,
/// which is 128 since mio 1.1. Tests open many concurrent connections and on
/// Windows connections are reset once the accept queue is full.
pub async fn bind_tcp_listener(
  addr: std::net::SocketAddr,
) -> std::io::Result<tokio::net::TcpListener> {
  let socket = if addr.is_ipv4() {
    tokio::net::TcpSocket::new_v4()?
  } else {
    tokio::net::TcpSocket::new_v6()?
  };
  #[cfg(not(windows))]
  socket.set_reuseaddr(true)?;
  socket.bind(addr)?;
  socket.listen(1024)
}

fn setup_panic_hook() {
  // Tokio does not exit the process when a task panics, so we define a custom
  // panic hook to implement this behaviour.
  let orig_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |panic_info| {
    std::eprintln!(
      "\n============================================================"
    );
    std::eprintln!("Test server panicked!\n");
    orig_hook(panic_info);
    std::process::exit(1);
  }));
}
