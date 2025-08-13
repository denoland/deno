use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::rc::Rc;
use std::sync::Arc;

// Copyright 2018-2025 the Deno authors. MIT license.
use base64::Engine;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToJsBuffer;
use deno_core::op2;
use deno_core::serde_v8::to_v8;
use deno_core::unsync::spawn;
use deno_core::v8;
use deno_core::v8_static_strings;
use deno_net::io::TcpStreamResource;
use deno_net::ops_tls::TlsStreamResource;
use deno_net::tunnel::quinn::crypto::rustls;
use once_cell::unsync::OnceCell;
use rustls_tokio_stream::TlsStream;
use rustls_tokio_stream::rustls::ClientConfig;
use rustls_tokio_stream::rustls::ClientConnection;
use rustls_tokio_stream::rustls::RootCertStore;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use webpki_root_certs;

use super::crypto::x509::Certificate;
use super::crypto::x509::CertificateObject;

#[op2]
#[serde]
pub fn op_get_root_certificates() -> Vec<String> {
  let certs = webpki_root_certs::TLS_SERVER_ROOT_CERTS
    .iter()
    .map(|cert| {
      let b64 = base64::engine::general_purpose::STANDARD.encode(cert);
      let pem_lines = b64
        .chars()
        .collect::<Vec<char>>()
        // Node uses 72 characters per line, so we need to follow node even though
        // it's not spec compliant https://datatracker.ietf.org/doc/html/rfc7468#section-2
        .chunks(72)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("\n");
      let pem = format!(
        "-----BEGIN CERTIFICATE-----\n{pem_lines}\n-----END CERTIFICATE-----\n",
      );
      pem
    })
    .collect::<Vec<String>>();
  certs
}

#[op2]
#[serde]
pub fn op_tls_peer_certificate(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  detailed: bool,
) -> Option<CertificateObject> {
  let resource = state.resource_table.get::<TlsStreamResource>(rid).ok()?;
  let certs = resource.peer_certificates()?;

  if certs.is_empty() {
    return None;
  }

  // For Node.js compatibility, return the peer certificate (first in chain)
  let cert_der = &certs[0];

  let cert = Certificate::from_der(cert_der.as_ref()).ok()?;
  cert.to_object(detailed).ok()
}

#[op2]
#[string]
pub fn op_tls_canonicalize_ipv4_address(
  #[string] hostname: String,
) -> Option<String> {
  let ip = hostname.parse::<std::net::IpAddr>().ok()?;

  let canonical_ip = match ip {
    std::net::IpAddr::V4(ipv4) => ipv4.to_string(),
    std::net::IpAddr::V6(ipv6) => ipv6.to_string(),
  };

  Some(canonical_ip)
}

enum SslImpl {
  Rustls(TlsStream<TcpStream>),
}

pub struct TLSWrap {
  stream: v8::Global<v8::Object>,
  _is_server: bool,
  _has_active_from_prev_owner: bool,
  ssl: Rc<UnsafeCell<Option<SslImpl>>>,
}

#[op2]
#[cppgc]
pub fn op_tls_wrap(
  #[global] stream: v8::Global<v8::Object>,
  _context: v8::Local<v8::Object>,
  is_server: bool,
  has_active_from_prev_owner: bool,
) -> TLSWrap {
  TLSWrap {
    ssl: Default::default(),
    stream,
    _is_server: is_server,
    _has_active_from_prev_owner: has_active_from_prev_owner,
  }
}

impl GarbageCollected for TLSWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TLSWrap"
  }
}

#[op2]
impl TLSWrap {
  #[fast]
  fn start(&self, scope: &mut v8::HandleScope, state: &mut OpState) {
    v8_static_strings! {
      RID = "_rid",
    }

    let rid_str = RID.v8_string(scope).unwrap();
    let stream = v8::Local::new(scope, &self.stream);
    let rid = stream
      .get(scope, rid_str.into())
      .expect("Failed to get resource ID")
      .int32_value(scope)
      .expect("Failed to convert resource ID to integer");

    let resource = state
      .resource_table
      .take::<TcpStreamResource>(rid as _)
      .expect("Failed to take resource from OpState");
    let resource = Rc::try_unwrap(resource).unwrap();
    let (read_half, write_half) = resource.into_inner();
    let tcp_stream = read_half
      .reunite(write_half)
      .expect("Failed to reunite halves");

    let config = ClientConfig::builder()
      .with_root_certificates(Arc::new(
        deno_tls::create_default_root_cert_store(),
      ))
      .with_no_client_auth();

    let tls_config = Arc::new(config);
    let ssl = TlsStream::new_client_side(
      tcp_stream,
      ClientConnection::new(
        tls_config,
        "google.com".try_into().expect("Invalid hostname"),
      )
      .unwrap(),
      None,
    );

    unsafe { *self.ssl.get() = Some(SslImpl::Rustls(ssl)) }
  }

  #[async_method]
  async fn handshake(&self) {
    let mut ssl = unsafe { &mut *self.ssl.get() };
    let mut ssl = ssl.as_mut().expect("SSL implementation not initialized");
    let ssl = match ssl {
      SslImpl::Rustls(ssl) => ssl,
    };

    std::pin::Pin::new(ssl)
      .handshake()
      .await
      .expect("Failed to complete SSL handshake");
  }

  fn write_utf8_string(
    &self,
    #[global] _: v8::Global<v8::Object>,
    #[string] data: String,
  ) -> u32 {
    let ssl = self.ssl.clone();
    spawn(async move {
      let mut ssl = unsafe { &mut *ssl.get() };
      let ssl = ssl.as_mut().expect("SSL implementation not initialized");
      let ssl = match ssl {
        SslImpl::Rustls(ssl) => ssl,
      };

      let data = data.as_bytes();
      let n = ssl
        .write(data)
        .await
        .expect("Failed to write to SSL stream");
    });

    0
  }

  fn write_buffer(
    &self,
    #[global] _: v8::Global<v8::Object>,
    #[buffer] data: JsBuffer,
  ) -> u32 {
    let ssl = self.ssl.clone();
    spawn(async move {
      let mut ssl = unsafe { &mut *ssl.get() };
      let ssl = ssl.as_mut().expect("SSL implementation not initialized");
      let ssl = match ssl {
        SslImpl::Rustls(ssl) => ssl,
      };

      let data = data.as_ref();
      let n = ssl
        .write(data)
        .await
        .expect("Failed to write to SSL stream");
    });
    0
  }

  #[async_method]
  async fn read_start(
    &self,
    state: Rc<RefCell<OpState>>,
    #[this] this: v8::Global<v8::Object>,
  ) {
    let mut ssl = unsafe { &mut *self.ssl.get() };
    let ssl = ssl.as_mut().expect("SSL implementation not initialized");
    let ssl = match ssl {
      SslImpl::Rustls(ssl) => ssl,
    };

    let mut buf = vec![0; 4096];
    let nread = match ssl
      .read(&mut buf)
      .await {
      Ok(n) => n,
      Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
        0
      }
      Err(e) => {
        eprintln!("Error reading from SSL stream: {:?}", e);
        return;
      }
    };

    state
      .borrow()
      .borrow::<deno_core::V8TaskSpawner>()
      .spawn(move |scope| {
        let stream = v8::Local::new(scope, &this);
        v8_static_strings! {
            ONREAD = "onread",
            READ_START = "readStart",
        }

        let onread_str = ONREAD.v8_string(scope).unwrap();
        let onread = stream
          .get(scope, onread_str.into())
          .expect("Failed to get onread function");

        let onread = v8::Local::<v8::Function>::try_from(onread)
          .expect("Failed to convert onread to Function");

        let buf = ToJsBuffer::from(buf[..nread].to_vec());
        let buf = deno_core::serde_v8::to_v8(scope, &buf).unwrap();
        let nread_v = v8::Number::new(scope, nread as f64);
        onread.call(scope, stream.into(), &[buf, nread_v.into()]);
      });
  }

  #[fast]
  fn read_stop(&self) {}

  #[fast]
  fn set_verify_mode(&self) {}

  #[fast]
  fn set_net_perm_token(&self) {}
}
