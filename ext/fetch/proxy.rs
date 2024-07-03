// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use deno_core::futures::TryFutureExt;
use deno_tls::rustls::ClientConfig as TlsConfig;

use http::header::HeaderValue;
use http::uri::Scheme;
use http::Uri;
use hyper_util::client::legacy::connect::Connected;
use hyper_util::client::legacy::connect::Connection;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
use tokio_socks::tcp::Socks5Stream;
use tower_service::Service;

#[derive(Debug, Clone)]
pub(crate) struct ProxyConnector<C> {
  connector: C,
  proxies: Arc<[Intercept]>,
  tls: Arc<TlsConfig>,
  user_agent: Option<HeaderValue>,
}

#[derive(Clone)]
pub(crate) struct Intercept {
  filter: Filter,
  target: Target,
}

#[derive(Clone)]
enum Target {
  Http {
    dst: Uri,
    auth: Option<HeaderValue>,
  },
  Https {
    dst: Uri,
    auth: Option<HeaderValue>,
  },
  Socks {
    dst: Uri,
    auth: Option<(String, String)>,
  },
}

#[derive(Debug, Clone, Copy)]
enum Filter {
  Http,
  Https,
  All,
}

pub(crate) fn from_env() -> Vec<Intercept> {
  let mut proxies = Vec::new();

  if let Some(proxy) = parse_env_var("ALL_PROXY", Filter::All) {
    proxies.push(proxy);
  } else if let Some(proxy) = parse_env_var("all_proxy", Filter::All) {
    proxies.push(proxy);
  }

  if let Some(proxy) = parse_env_var("HTTPS_PROXY", Filter::Https) {
    proxies.push(proxy);
  } else if let Some(proxy) = parse_env_var("https_proxy", Filter::Https) {
    proxies.push(proxy);
  }

  // In a CGI context, headers become environment variables. So, "Proxy:" becomes HTTP_PROXY.
  // To prevent an attacker from injecting a proxy, check if we are in CGI.
  if env::var_os("REQUEST_METHOD").is_none() {
    if let Some(proxy) = parse_env_var("HTTP_PROXY", Filter::Http) {
      proxies.push(proxy);
    } else if let Some(proxy) = parse_env_var("http_proxy", Filter::Https) {
      proxies.push(proxy);
    }
  }

  proxies
}

pub(crate) fn basic_auth(user: &str, pass: &str) -> HeaderValue {
  use base64::prelude::BASE64_STANDARD;
  use base64::write::EncoderWriter;
  use std::io::Write;

  let mut buf = b"Basic ".to_vec();
  {
    let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
    let _ = write!(encoder, "{user}:{pass}");
  }
  let mut header =
    HeaderValue::from_bytes(&buf).expect("base64 is always valid HeaderValue");
  header.set_sensitive(true);
  header
}

fn parse_env_var(name: &str, filter: Filter) -> Option<Intercept> {
  let val = env::var(name).ok()?;
  let target = Target::parse(&val)?;
  Some(Intercept { filter, target })
}

impl Intercept {
  pub(crate) fn all(s: &str) -> Option<Self> {
    let target = Target::parse(s)?;
    Some(Intercept {
      filter: Filter::All,
      target,
    })
  }

  pub(crate) fn set_auth(&mut self, user: &str, pass: &str) {
    match self.target {
      Target::Http { ref mut auth, .. } => {
        *auth = Some(basic_auth(user, pass));
      }
      Target::Https { ref mut auth, .. } => {
        *auth = Some(basic_auth(user, pass));
      }
      Target::Socks { ref mut auth, .. } => {
        *auth = Some((user.into(), pass.into()));
      }
    }
  }
}

impl std::fmt::Debug for Intercept {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Intercept")
      .field("filter", &self.filter)
      .finish()
  }
}

impl Target {
  fn parse(val: &str) -> Option<Self> {
    let uri = val.parse::<Uri>().ok()?;

    let mut builder = Uri::builder();
    let mut is_socks = false;
    let mut http_auth = None;
    let mut socks_auth = None;

    builder = builder.scheme(match uri.scheme() {
      Some(s) => {
        if s == &Scheme::HTTP || s == &Scheme::HTTPS {
          s.clone()
        } else if s.as_str() == "socks5" || s.as_str() == "socks5h" {
          is_socks = true;
          s.clone()
        } else {
          // can't use this proxy scheme
          return None;
        }
      }
      // if no scheme provided, assume they meant 'http'
      None => Scheme::HTTP,
    });

    let authority = uri.authority()?;

    if let Some((userinfo, host_port)) = authority.as_str().split_once('@') {
      let (user, pass) = userinfo.split_once(':')?;
      if is_socks {
        socks_auth = Some((user.into(), pass.into()));
      } else {
        http_auth = Some(basic_auth(user, pass));
      }
      builder = builder.authority(host_port);
    } else {
      builder = builder.authority(authority.clone());
    }

    // removing any path, but we MUST specify one or the builder errors
    builder = builder.path_and_query("/");

    let dst = builder.build().ok()?;

    let target = match dst.scheme().unwrap().as_str() {
      "https" => Target::Https {
        dst,
        auth: http_auth,
      },
      "http" => Target::Http {
        dst,
        auth: http_auth,
      },
      "socks5" | "socks5h" => Target::Socks {
        dst,
        auth: socks_auth,
      },
      // shouldn't happen
      _ => return None,
    };

    Some(target)
  }
}

impl<C> ProxyConnector<C> {
  pub(crate) fn new<I>(intercepts: I, connector: C, tls: Arc<TlsConfig>) -> Self
  where
    Arc<[Intercept]>: From<I>,
  {
    ProxyConnector {
      connector,
      proxies: Arc::from(intercepts),
      tls,
      user_agent: None,
    }
  }

  pub(crate) fn user_agent(&mut self, val: HeaderValue) {
    self.user_agent = Some(val);
  }

  fn intercept(&self, dst: &Uri) -> Option<&Intercept> {
    for intercept in &*self.proxies {
      return match (
        intercept.filter,
        dst.scheme().map(Scheme::as_str).unwrap_or(""),
      ) {
        (Filter::All, _) => Some(intercept),
        (Filter::Https, "https") => Some(intercept),
        (Filter::Http, "http") => Some(intercept),
        _ => continue,
      };
    }
    None
  }
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

// These variatns are not to be inspected.
pub enum Proxied<T> {
  /// Not proxied
  PassThrough(T),
  /// An HTTP forwarding proxy needed absolute-form
  HttpForward(T),
  /// Tunneled through HTTP CONNECT
  HttpTunneled(Box<TokioIo<TlsStream<TokioIo<T>>>>),
  /// Tunneled through SOCKS
  Socks(TokioIo<TcpStream>),
  /// Tunneled through SOCKS and TLS
  SocksTls(TokioIo<TlsStream<TokioIo<TokioIo<TcpStream>>>>),
}

impl<C> Service<Uri> for ProxyConnector<C>
where
  C: Service<Uri>,
  C::Response: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
  C::Future: Send + 'static,
  C::Error: Into<BoxError> + 'static,
{
  type Response = Proxied<C::Response>;
  type Error = BoxError;
  type Future = BoxFuture<Result<Self::Response, Self::Error>>;

  fn poll_ready(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    self.connector.poll_ready(cx).map_err(Into::into)
  }

  fn call(&mut self, orig_dst: Uri) -> Self::Future {
    if let Some(intercept) = self.intercept(&orig_dst).cloned() {
      let is_https = orig_dst.scheme() == Some(&Scheme::HTTPS);
      let user_agent = self.user_agent.clone();
      return match intercept.target {
        Target::Http {
          dst: proxy_dst,
          auth,
        }
        | Target::Https {
          dst: proxy_dst,
          auth,
        } => {
          let connecting = self.connector.call(proxy_dst);
          let tls = TlsConnector::from(self.tls.clone());
          Box::pin(async move {
            let mut io = connecting.await.map_err(Into::into)?;

            if is_https {
              tunnel(&mut io, &orig_dst, user_agent, auth).await?;
              let tokio_io = TokioIo::new(io);
              let io = tls
                .connect(
                  TryFrom::try_from(orig_dst.host().unwrap().to_owned())?,
                  tokio_io,
                )
                .await?;
              Ok(Proxied::HttpTunneled(Box::new(TokioIo::new(io))))
            } else {
              Ok(Proxied::HttpForward(io))
            }
          })
        }
        Target::Socks {
          dst: proxy_dst,
          auth,
        } => {
          let tls = TlsConnector::from(self.tls.clone());
          Box::pin(async move {
            let socks_addr = (
              proxy_dst.host().unwrap(),
              proxy_dst.port().map(|p| p.as_u16()).unwrap_or(1080),
            );
            let host = orig_dst.host().ok_or("no host in url")?;
            let port = match orig_dst.port() {
              Some(p) => p.as_u16(),
              None if is_https => 443,
              _ => 80,
            };
            let io = if let Some((user, pass)) = auth {
              Socks5Stream::connect_with_password(
                socks_addr,
                (host, port),
                &user,
                &pass,
              )
              .await?
            } else {
              Socks5Stream::connect(socks_addr, (host, port)).await?
            };
            let io = TokioIo::new(io.into_inner());

            if is_https {
              let tokio_io = TokioIo::new(io);
              let io = tls
                .connect(TryFrom::try_from(host.to_owned())?, tokio_io)
                .await?;
              Ok(Proxied::SocksTls(TokioIo::new(io)))
            } else {
              Ok(Proxied::Socks(io))
            }
          })
        }
      };
    }
    Box::pin(
      self
        .connector
        .call(orig_dst)
        .map_ok(Proxied::PassThrough)
        .map_err(Into::into),
    )
  }
}

async fn tunnel<T>(
  io: &mut T,
  dst: &Uri,
  user_agent: Option<HeaderValue>,
  auth: Option<HeaderValue>,
) -> Result<(), BoxError>
where
  T: hyper::rt::Read + hyper::rt::Write + Unpin,
{
  use tokio::io::AsyncReadExt;
  use tokio::io::AsyncWriteExt;

  let host = dst.host().expect("proxy dst has host");
  let port = match dst.port() {
    Some(p) => p.as_u16(),
    None => match dst.scheme().map(Scheme::as_str).unwrap_or("") {
      "https" => 443,
      "http" => 80,
      _ => return Err("proxy dst unexpected scheme".into()),
    },
  };

  let mut buf = format!(
    "\
     CONNECT {host}:{port} HTTP/1.1\r\n\
     Host: {host}:{port}\r\n\
     "
  )
  .into_bytes();

  // user-agent
  if let Some(user_agent) = user_agent {
    buf.extend_from_slice(b"User-Agent: ");
    buf.extend_from_slice(user_agent.as_bytes());
    buf.extend_from_slice(b"\r\n");
  }

  // proxy-authorization
  if let Some(value) = auth {
    buf.extend_from_slice(b"Proxy-Authorization: ");
    buf.extend_from_slice(value.as_bytes());
    buf.extend_from_slice(b"\r\n");
  }

  // headers end
  buf.extend_from_slice(b"\r\n");

  let mut tokio_conn = TokioIo::new(io);

  tokio_conn.write_all(&buf).await?;

  let mut buf = [0; 64];
  let mut pos = 0;

  loop {
    let n = tokio_conn.read(&mut buf[pos..]).await?;

    if n == 0 {
      return Err("unexpected eof while tunneling".into());
    }
    pos += n;

    let recvd = &buf[..pos];
    if recvd.starts_with(b"HTTP/1.1 200") || recvd.starts_with(b"HTTP/1.0 200")
    {
      if recvd.ends_with(b"\r\n\r\n") {
        return Ok(());
      }
      if pos == buf.len() {
        return Err("proxy headers too long for tunnel".into());
      }
    // else read more
    } else if recvd.starts_with(b"HTTP/1.1 407") {
      return Err("proxy authentication required".into());
    } else {
      return Err("unsuccessful tunnel".into());
    }
  }
}

impl<T> hyper::rt::Read for Proxied<T>
where
  T: hyper::rt::Read + hyper::rt::Write + Unpin,
{
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: hyper::rt::ReadBufCursor<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    match *self {
      Proxied::PassThrough(ref mut p) => Pin::new(p).poll_read(cx, buf),
      Proxied::HttpForward(ref mut p) => Pin::new(p).poll_read(cx, buf),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_read(cx, buf),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_read(cx, buf),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_read(cx, buf),
    }
  }
}

impl<T> hyper::rt::Write for Proxied<T>
where
  T: hyper::rt::Read + hyper::rt::Write + Unpin,
{
  fn poll_write(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<Result<usize, std::io::Error>> {
    match *self {
      Proxied::PassThrough(ref mut p) => Pin::new(p).poll_write(cx, buf),
      Proxied::HttpForward(ref mut p) => Pin::new(p).poll_write(cx, buf),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_write(cx, buf),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_write(cx, buf),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_write(cx, buf),
    }
  }

  fn poll_flush(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    match *self {
      Proxied::PassThrough(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::HttpForward(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_flush(cx),
    }
  }

  fn poll_shutdown(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    match *self {
      Proxied::PassThrough(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::HttpForward(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_shutdown(cx),
    }
  }

  fn is_write_vectored(&self) -> bool {
    match *self {
      Proxied::PassThrough(ref p) => p.is_write_vectored(),
      Proxied::HttpForward(ref p) => p.is_write_vectored(),
      Proxied::HttpTunneled(ref p) => p.is_write_vectored(),
      Proxied::Socks(ref p) => p.is_write_vectored(),
      Proxied::SocksTls(ref p) => p.is_write_vectored(),
    }
  }

  fn poll_write_vectored(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    bufs: &[std::io::IoSlice<'_>],
  ) -> Poll<Result<usize, std::io::Error>> {
    match *self {
      Proxied::PassThrough(ref mut p) => {
        Pin::new(p).poll_write_vectored(cx, bufs)
      }
      Proxied::HttpForward(ref mut p) => {
        Pin::new(p).poll_write_vectored(cx, bufs)
      }
      Proxied::HttpTunneled(ref mut p) => {
        Pin::new(p).poll_write_vectored(cx, bufs)
      }
      Proxied::Socks(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
    }
  }
}

impl<T> Connection for Proxied<T>
where
  T: Connection,
{
  fn connected(&self) -> Connected {
    match self {
      Proxied::PassThrough(ref p) => p.connected(),
      Proxied::HttpForward(ref p) => p.connected().proxy(true),
      Proxied::HttpTunneled(ref p) => p.inner().get_ref().0.connected(),
      Proxied::Socks(ref p) => p.connected(),
      Proxied::SocksTls(ref p) => p.inner().get_ref().0.connected(),
    }
  }
}

#[test]
fn test_proxy_parse_from_env() {
  fn parse(s: &str) -> Target {
    Target::parse(s).unwrap()
  }

  // normal
  match parse("http://127.0.0.1:6666") {
    Target::Http { dst, auth } => {
      assert_eq!(dst, "http://127.0.0.1:6666");
      assert!(auth.is_none());
    }
    _ => panic!("bad target"),
  }

  // without scheme
  match parse("127.0.0.1:6666") {
    Target::Http { dst, auth } => {
      assert_eq!(dst, "http://127.0.0.1:6666");
      assert!(auth.is_none());
    }
    _ => panic!("bad target"),
  }

  // with userinfo
  match parse("user:pass@127.0.0.1:6666") {
    Target::Http { dst, auth } => {
      assert_eq!(dst, "http://127.0.0.1:6666");
      assert!(auth.is_some());
      assert!(auth.unwrap().is_sensitive());
    }
    _ => panic!("bad target"),
  }

  // socks
  match parse("socks5://user:pass@127.0.0.1:6666") {
    Target::Socks { dst, auth } => {
      assert_eq!(dst, "socks5://127.0.0.1:6666");
      assert!(auth.is_some());
    }
    _ => panic!("bad target"),
  }

  // socks5h
  match parse("socks5h://localhost:6666") {
    Target::Socks { dst, auth } => {
      assert_eq!(dst, "socks5h://localhost:6666");
      assert!(auth.is_none());
    }
    _ => panic!("bad target"),
  }
}
