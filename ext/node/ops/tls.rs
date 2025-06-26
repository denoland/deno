use std::sync::Arc;

// Copyright 2018-2025 the Deno authors. MIT license.
use base64::Engine;
use deno_core::op2;
use deno_core::v8;
use deno_core::OpState;
use deno_net::ops_tls::TLS_BUFFER_SIZE;
use deno_tls::create_client_config;
use deno_tls::rustls;
use deno_tls::rustls::pki_types::ServerName;
use deno_tls::rustls::ClientConnection;
use deno_tls::SocketUse;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use deno_tls::TlsStream;
use tokio::io::AsyncRead;
use webpki_root_certs;

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

pub struct JSStream {}

impl deno_core::GarbageCollected for JSStream {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"JSStream"
  }
}

#[op2(stack_trace)]
pub fn op_tls_start_jsstream(
  state: &mut OpState,
  #[cppgc] stream: &JSStream,
  #[serde] ca_certs: Vec<String>,
  #[string] hostname: &str,
  #[serde] alpn_protocols: Option<Vec<String>>,
  reject_unauthorized: Option<bool>,
  #[cppgc] key_pair: Option<&TlsKeysHolder>,
) {
  let reject_unauthorized = reject_unauthorized.unwrap_or(true);
  let hostname = match hostname {
    "" => "localhost".to_string(),
    n => n.to_string(),
  };

  let ca_certs = ca_certs
    .into_iter()
    .map(|s| s.into_bytes())
    .collect::<Vec<_>>();

  let hostname_dns = ServerName::try_from(hostname.to_string()).unwrap();

  // --unsafely-ignore-certificate-errors overrides the `rejectUnauthorized` option.
  let unsafely_ignore_certificate_errors = if reject_unauthorized {
    state
      .try_borrow::<deno_net::UnsafelyIgnoreCertificateErrors>()
      .and_then(|it| it.0.clone())
  } else {
    Some(Vec::new())
  };

  let root_cert_store = state
    .borrow::<deno_net::DefaultTlsOptions>()
    .root_cert_store()
    .unwrap();

  let tls_null = TlsKeysHolder::from(TlsKeys::Null);
  let key_pair = key_pair.unwrap_or(&tls_null);
  let mut tls_config = create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    key_pair.take(),
    SocketUse::GeneralSsl,
  )
  .unwrap();
  if let Some(alpn_protocols) = alpn_protocols {
    tls_config.alpn_protocols =
      alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
  }

  let tls_config = Arc::new(tls_config);
  let tls_stream = TlsStream::new_client_side(
    stream,
    ClientConnection::new(tls_config, hostname_dns)?,
    TLS_BUFFER_SIZE,
  );

  todo!()
}

// impl deno_net::rustls_tokio_stream::UnderlyingStream for JSStream {
//
// }

struct ReadFuture {
  onread: v8::Global<v8::Function>,
}

impl std::future::Future for ReadFuture {
  type Output = ();

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    std::task::Poll::Pending
  }
}

// Rust wants to start a JSStream
impl AsyncRead for JSStream {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    std::task::Poll::Pending
  }
}

fn get_function_global(
  scope: &mut v8::HandleScope,
  this: v8::Global<v8::Object>,
  name: &str,
) -> v8::Global<v8::Function> {
  let name_str = v8::String::new(scope, name).unwrap();
  let this = v8::Local::new(scope, this);
  let function = this.get(scope, name_str.into()).unwrap();

  let function =
    v8::Local::<v8::Function>::try_from(function).expect("Expected a function");
  v8::Global::new(scope, function)
}

#[op2]
impl JSStream {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> JSStream {
    JSStream {}
  }

  // JS wants to start a JSStream
  #[fast]
  fn read_start(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope,
  ) {
    let fut = ReadFuture {
      onread: get_function_global(scope, this, "onreadstart"),
    };
    deno_unsync::spawn(async move {
      fut.await;
    });
  }

  #[fast]
  fn read_stop(&self) {}

  #[fast]
  fn shutdown(&self) {}

  #[fast]
  fn use_user_buffer(&self) {}
  #[fast]
  fn write_buffer(&self) {}
  #[fast]
  fn writev(&self) {}
}
