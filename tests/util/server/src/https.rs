// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use anyhow::anyhow;
use futures::Stream;
use futures::StreamExt;
use rustls_tokio_stream::rustls;
use rustls_tokio_stream::rustls::pki_types::CertificateDer;
use rustls_tokio_stream::rustls::pki_types::PrivateKeyDer;
use rustls_tokio_stream::TlsStream;
use std::io;
use std::num::NonZeroUsize;
use std::result::Result;
use std::sync::Arc;
use tokio::net::TcpStream;

use crate::get_tcp_listener_stream;
use crate::testdata_path;

pub const TLS_BUFFER_SIZE: Option<NonZeroUsize> = NonZeroUsize::new(65536);

#[derive(Default)]
pub enum SupportedHttpVersions {
  #[default]
  All,
  Http1Only,
  Http2Only,
}

pub fn get_tls_listener_stream_from_tcp(
  tls_config: Arc<rustls::ServerConfig>,
  mut tcp: impl Stream<Item = Result<TcpStream, std::io::Error>> + Unpin + 'static,
) -> impl Stream<Item = Result<TlsStream, std::io::Error>> + Unpin {
  async_stream::stream! {
    while let Some(result) = tcp.next().await {
      match result {
        Ok(tcp) => yield Ok(TlsStream::new_server_side(tcp, tls_config.clone(), TLS_BUFFER_SIZE)),
        Err(e) => yield Err(e),
      };
    }
  }.boxed_local()
}

pub async fn get_tls_listener_stream(
  name: &'static str,
  port: u16,
  http: SupportedHttpVersions,
) -> impl Stream<Item = Result<TlsStream, std::io::Error>> + Unpin {
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";
  let tls_config =
    get_tls_config(cert_file, key_file, ca_cert_file, http).unwrap();

  let tcp = get_tcp_listener_stream(name, port).await;
  get_tls_listener_stream_from_tcp(tls_config, tcp)
}

pub fn get_tls_config(
  cert: &str,
  key: &str,
  ca: &str,
  http_versions: SupportedHttpVersions,
) -> io::Result<Arc<rustls::ServerConfig>> {
  let cert_path = testdata_path().join(cert);
  let key_path = testdata_path().join(key);
  let ca_path = testdata_path().join(ca);

  let cert_file = std::fs::File::open(cert_path)?;
  let key_file = std::fs::File::open(key_path)?;
  let ca_file = std::fs::File::open(ca_path)?;

  let certs_result: Result<Vec<CertificateDer<'static>>, io::Error> = {
    let mut cert_reader = io::BufReader::new(cert_file);
    rustls_pemfile::certs(&mut cert_reader).collect()
  };
  let certs = certs_result?;

  let mut ca_cert_reader = io::BufReader::new(ca_file);
  let ca_cert = rustls_pemfile::certs(&mut ca_cert_reader)
    .collect::<Vec<_>>()
    .remove(0)?;

  let mut key_reader = io::BufReader::new(key_file);
  let key = {
    let pkcs8_keys = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
      .collect::<Result<Vec<_>, _>>()?;
    let rsa_keys = rustls_pemfile::rsa_private_keys(&mut key_reader)
      .collect::<Result<Vec<_>, _>>()?;

    if !pkcs8_keys.is_empty() {
      let key = pkcs8_keys[0].clone_key();
      Some(PrivateKeyDer::from(key))
    } else if !rsa_keys.is_empty() {
      let key = rsa_keys[0].clone_key();
      Some(PrivateKeyDer::from(key))
    } else {
      None
    }
  };

  match key {
    Some(key) => {
      let mut root_cert_store = rustls::RootCertStore::empty();
      root_cert_store.add(ca_cert).unwrap();

      // Allow (but do not require) client authentication.
      let client_verifier = rustls::server::WebPkiClientVerifier::builder(
        Arc::new(root_cert_store),
      )
      .allow_unauthenticated()
      .build()
      .unwrap();

      let mut config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, key)
        .map_err(|e| anyhow!("Error setting cert: {:?}", e))
        .unwrap();

      match http_versions {
        SupportedHttpVersions::All => {
          config.alpn_protocols = vec!["h2".into(), "http/1.1".into()];
        }
        SupportedHttpVersions::Http1Only => {}
        SupportedHttpVersions::Http2Only => {
          config.alpn_protocols = vec!["h2".into()];
        }
      }

      Ok(Arc::new(config))
    }
    None => Err(io::Error::new(io::ErrorKind::Other, "Cannot find key")),
  }
}
