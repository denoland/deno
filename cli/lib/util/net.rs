// Copyright 2018-2026 the Deno authors. MIT license.

/// Bind to an ephemeral TCP port on the loopback interface and return the
/// number assigned by the kernel. The listener is dropped before returning,
/// so the caller is racing against any other process for the port — on
/// loopback the window is small enough to be acceptable for ad-hoc service
/// ports (DevTools mux, desktop HTTP server, …).
pub fn allocate_random_port() -> std::io::Result<u16> {
  let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
  Ok(listener.local_addr()?.port())
}
