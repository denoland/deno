// Copyright 2024 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use deno_core::futures::TryFutureExt;

use http::header::HeaderValue;
use http::uri::Scheme;
use http::Uri;
use tower_service::Service;

#[derive(Debug, Clone)]
pub(crate) struct ProxyConnector<C> {
  connector: C,
  proxies: Arc<[Intercept]>,
  user_agent: Option<HeaderValue>,
}

#[derive(Debug, Clone)]
pub(crate) struct Intercept {
  filter: Filter,
  dst: Uri,
  auth: Option<HeaderValue>,
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
  Intercept::parse_with(filter, &val)
}

impl Intercept {
  pub(crate) fn all(dst: Uri) -> Self {
    Self {
      filter: Filter::All,
      dst,
      auth: None,
    }
  }

  pub(crate) fn basic_auth(&mut self, user: &str, pass: &str) {
    self.auth = Some(basic_auth(user, pass));
  }

  fn parse_with(filter: Filter, val: &str) -> Option<Self> {
    let uri = val.parse::<Uri>().ok()?;

    let mut builder = Uri::builder();
    let mut auth = None;

    builder = builder.scheme(match uri.scheme() {
      Some(s) => {
        if s == &Scheme::HTTP || s == &Scheme::HTTPS {
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
      auth = Some(basic_auth(user, pass));
      builder = builder.authority(host_port);
    } else {
      builder = builder.authority(authority.clone());
    }

    // removing any path, but we MUST specify one or the builder errors
    builder = builder.path_and_query("/");

    let dst = builder.build().ok()?;

    Some(Intercept { filter, dst, auth })
  }
}

impl<C> ProxyConnector<C> {
  pub(crate) fn new<I>(intercepts: I, connector: C) -> Self
  where
    Arc<[Intercept]>: From<I>,
  {
    ProxyConnector {
      connector,
      proxies: Arc::from(intercepts),
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

impl<C> Service<Uri> for ProxyConnector<C>
where
  C: Service<Uri>,
  C::Response: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
  C::Future: Send + 'static,
  C::Error: Into<BoxError> + 'static,
{
  type Response = C::Response;
  type Error = BoxError;
  type Future = BoxFuture<Result<Self::Response, Self::Error>>;

  fn poll_ready(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    self.connector.poll_ready(cx).map_err(Into::into)
  }

  fn call(&mut self, dst: Uri) -> Self::Future {
    if let Some(intercept) = self.intercept(&dst) {
      let user_agent = self.user_agent.clone();
      let auth = intercept.auth.clone();
      let connecting = self.connector.call(intercept.dst.clone());
      return Box::pin(async move {
        let mut io = connecting.await.map_err(Into::into)?;
        tunnel(&mut io, dst, user_agent, auth).await?;
        Ok(io)
      });
    }
    Box::pin(self.connector.call(dst).map_err(Into::into))
  }
}

async fn tunnel<T>(
  io: &mut T,
  dst: Uri,
  user_agent: Option<HeaderValue>,
  auth: Option<HeaderValue>,
) -> Result<(), BoxError>
where
  T: hyper::rt::Read + hyper::rt::Write + Unpin,
{
  use hyper_util::rt::TokioIo;
  use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

#[test]
fn test_proxy_from_env() {
  fn parse(s: &str) -> Intercept {
    Intercept::parse_with(Filter::All, s).unwrap()
  }

  let normal = parse("http://127.0.0.1:6666");
  assert_eq!(normal.dst, "http://127.0.0.1:6666");
  assert!(normal.auth.is_none());

  let without_scheme = parse("127.0.0.1:6666");
  assert_eq!(without_scheme.dst, "http://127.0.0.1:6666");

  let with_userinfo = parse("user:pass@127.0.0.1:6666");
  assert_eq!(with_userinfo.dst, "http://127.0.0.1:6666");
  assert!(with_userinfo.auth.is_some());
  assert!(with_userinfo.auth.unwrap().is_sensitive());
}
