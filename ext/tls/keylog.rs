// Copyright 2018-2026 the Deno authors. MIT license.

//! TLS key logging support for debugging encrypted traffic.
//!
//! When the `SSLKEYLOGFILE` environment variable is set, TLS session keys
//! are written to the specified file in NSS Key Log format, which can be
//! used by tools like Wireshark to decrypt TLS traffic.

use std::sync::Arc;
use std::sync::OnceLock;

use rustls::KeyLog;
use rustls::KeyLogFile;

static SSL_KEY_LOG: OnceLock<Arc<KeyLogFile>> = OnceLock::new();

pub fn get_ssl_key_log() -> Arc<dyn KeyLog> {
  SSL_KEY_LOG
    .get_or_init(|| Arc::new(KeyLogFile::new()))
    .clone()
}
