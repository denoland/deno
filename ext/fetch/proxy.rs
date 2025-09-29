// Copyright 2018-2025 the Deno authors. MIT license.

//! Parts of this module should be able to be replaced with other crates
//! eventually, once generic versions appear in hyper-util, et al.

use std::env;
use std::future::Future;
use std::net::IpAddr;
#[cfg(not(windows))]
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use deno_core::futures::TryFutureExt;
use deno_tls::rustls::ClientConfig as TlsConfig;
use http::Uri;
use http::header::HeaderValue;
use http::uri::Scheme;
use hyper_rustls::HttpsConnector;
use hyper_rustls::MaybeHttpsStream;
use hyper_util::client::legacy::connect::Connected;
use hyper_util::client::legacy::connect::Connection;
use hyper_util::rt::TokioIo;
use ipnet::IpNet;
use percent_encoding::percent_decode_str;
use tokio::net::TcpStream;
#[cfg(not(windows))]
use tokio::net::UnixStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tokio_socks::tcp::Socks5Stream;
#[cfg(any(
  target_os = "android",
  target_os = "linux",
  target_os = "macos"
))]
use tokio_vsock::VsockStream;
use tower_service::Service;

#[derive(Debug, Clone)]
pub(crate) struct ProxyConnector<C> {
  pub(crate) http: C,
  pub(crate) proxies: Arc<Proxies>,
  /// TLS config when destination is not a proxy
  pub(crate) tls: Arc<TlsConfig>,
  /// TLS config when destination is a proxy
  /// Notably, does not include ALPN
  pub(crate) tls_proxy: Arc<TlsConfig>,
  pub(crate) user_agent: Option<HeaderValue>,
}

impl<C> ProxyConnector<C> {
  pub(crate) fn h1_only(self) -> Option<ProxyConnector<C>>
  where
    C: Service<Uri>,
  {
    if !self.tls.alpn_protocols.is_empty() {
      self.tls.alpn_protocols.iter().find(|p| *p == b"http/1.1")?;
    }
    let mut tls = (*self.tls).clone();
    tls.alpn_protocols = vec![b"http/1.1".to_vec()];
    Some(ProxyConnector {
      http: self.http,
      proxies: self.proxies,
      tls: Arc::new(tls),
      tls_proxy: self.tls_proxy,
      user_agent: self.user_agent,
    })
  }

  pub(crate) fn h2_only(self) -> Option<ProxyConnector<C>>
  where
    C: Service<Uri>,
  {
    if !self.tls.alpn_protocols.is_empty() {
      self.tls.alpn_protocols.iter().find(|p| *p == b"h2")?;
    }
    let mut tls = (*self.tls).clone();
    tls.alpn_protocols = vec![b"h2".to_vec()];
    Some(ProxyConnector {
      http: self.http,
      proxies: self.proxies,
      tls: Arc::new(tls),
      tls_proxy: self.tls_proxy,
      user_agent: self.user_agent,
    })
  }
}

#[derive(Debug)]
pub(crate) struct Proxies {
  no: Option<NoProxy>,
  intercepts: Vec<Intercept>,
}

#[derive(Clone)]
pub(crate) struct Intercept {
  filter: Filter,
  target: Target,
}

#[derive(Clone)]
pub(crate) enum Target {
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
  Tcp {
    hostname: String,
    port: u16,
  },
  #[cfg(not(windows))]
  Unix {
    path: PathBuf,
  },
  #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
  Vsock {
    cid: u32,
    port: u32,
  },
}

#[derive(Debug, Clone, Copy)]
enum Filter {
  Http,
  Https,
  All,
}

pub(crate) fn from_env() -> Proxies {
  let mut intercepts = Vec::new();

  match parse_env_var("ALL_PROXY", Filter::All) {
    Some(proxy) => {
      intercepts.push(proxy);
    }
    _ => {
      if let Some(proxy) = parse_env_var("all_proxy", Filter::All) {
        intercepts.push(proxy);
      }
    }
  }

  match parse_env_var("HTTPS_PROXY", Filter::Https) {
    Some(proxy) => {
      intercepts.push(proxy);
    }
    _ => {
      if let Some(proxy) = parse_env_var("https_proxy", Filter::Https) {
        intercepts.push(proxy);
      }
    }
  }

  // In a CGI context, headers become environment variables. So, "Proxy:" becomes HTTP_PROXY.
  // To prevent an attacker from injecting a proxy, check if we are in CGI.
  if env::var_os("REQUEST_METHOD").is_none() {
    match parse_env_var("HTTP_PROXY", Filter::Http) {
      Some(proxy) => {
        intercepts.push(proxy);
      }
      _ => {
        if let Some(proxy) = parse_env_var("http_proxy", Filter::Http) {
          intercepts.push(proxy);
        }
      }
    }
  }

  let no = NoProxy::from_env();

  Proxies { intercepts, no }
}

pub fn basic_auth(user: &str, pass: Option<&str>) -> HeaderValue {
  use std::io::Write;

  use base64::prelude::BASE64_STANDARD;
  use base64::write::EncoderWriter;

  let mut buf = b"Basic ".to_vec();
  {
    let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
    let _ = write!(encoder, "{user}:");
    if let Some(password) = pass {
      let _ = write!(encoder, "{password}");
    }
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
  pub(crate) fn all(target: Target) -> Self {
    Intercept {
      filter: Filter::All,
      target,
    }
  }

  pub(crate) fn set_auth(&mut self, user: &str, pass: &str) {
    match self.target {
      Target::Http { ref mut auth, .. } => {
        *auth = Some(basic_auth(user, Some(pass)));
      }
      Target::Https { ref mut auth, .. } => {
        *auth = Some(basic_auth(user, Some(pass)));
      }
      Target::Socks { ref mut auth, .. } => {
        *auth = Some((user.into(), pass.into()));
      }
      Target::Tcp { .. } => {
        // Auth not supported for Tcp sockets
      }
      #[cfg(not(windows))]
      Target::Unix { .. } => {
        // Auth not supported for Unix sockets
      }
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Target::Vsock { .. } => {
        // Auth not supported for Vsock sockets
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
  pub(crate) fn parse(val: &str) -> Option<Self> {
    // unix:<path> is valid RFC3986 but not as an http::Uri
    #[cfg(not(windows))]
    if let Some(encoded_path) = val.strip_prefix("unix:") {
      use std::os::unix::ffi::OsStringExt;
      let decoded = std::ffi::OsString::from_vec(
        percent_decode_str(encoded_path).collect::<Vec<u8>>(),
      );
      return Some(Target::Unix {
        path: decoded.into(),
      });
    }
    // vsock:<cid>:<port> is valid RFC3986 but not as an http::Uri
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Some(cid_port) = val.strip_prefix("vsock:") {
      let (left, right) = cid_port.split_once(":")?;
      let cid = left.parse::<u32>().ok()?;
      let port = right.parse::<u32>().ok()?;
      return Some(Target::Vsock { cid, port });
    }

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
      let user = percent_decode_str(user).decode_utf8_lossy();
      let pass = percent_decode_str(pass).decode_utf8_lossy();
      if is_socks {
        socks_auth = Some((user.into(), pass.into()));
      } else {
        http_auth = Some(basic_auth(&user, Some(&pass)));
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

  pub(crate) fn new_tcp(hostname: String, port: u16) -> Self {
    Target::Tcp { hostname, port }
  }

  #[cfg(not(windows))]
  pub(crate) fn new_unix(path: PathBuf) -> Self {
    Target::Unix { path }
  }

  #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
  pub(crate) fn new_vsock(cid: u32, port: u32) -> Self {
    Target::Vsock { cid, port }
  }
}

#[derive(Debug)]
struct NoProxy {
  domains: DomainMatcher,
  ips: IpMatcher,
}

/// Represents a possible matching entry for an IP address
#[derive(Clone, Debug)]
enum Ip {
  Address(IpAddr),
  Network(IpNet),
}

/// A wrapper around a list of IP cidr blocks or addresses with a [IpMatcher::contains] method for
/// checking if an IP address is contained within the matcher
#[derive(Clone, Debug, Default)]
struct IpMatcher(Vec<Ip>);

/// A wrapper around a list of domains with a [DomainMatcher::contains] method for checking if a
/// domain is contained within the matcher
#[derive(Clone, Debug, Default)]
struct DomainMatcher(Vec<String>);

impl NoProxy {
  /// Returns a new no-proxy configuration based on environment variables (or `None` if no variables are set)
  /// see [self::NoProxy::from_string()] for the string format
  fn from_env() -> Option<NoProxy> {
    let raw = env::var("NO_PROXY")
      .or_else(|_| env::var("no_proxy"))
      .unwrap_or_default();

    Self::from_string(&raw)
  }

  /// Returns a new no-proxy configuration based on a `no_proxy` string (or `None` if no variables
  /// are set)
  /// The rules are as follows:
  /// * The environment variable `NO_PROXY` is checked, if it is not set, `no_proxy` is checked
  /// * If neither environment variable is set, `None` is returned
  /// * Entries are expected to be comma-separated (whitespace between entries is ignored)
  /// * IP addresses (both IPv4 and IPv6) are allowed, as are optional subnet masks (by adding /size,
  ///   for example "`192.168.1.0/24`").
  /// * An entry "`*`" matches all hostnames (this is the only wildcard allowed)
  /// * Any other entry is considered a domain name (and may contain a leading dot, for example `google.com`
  ///   and `.google.com` are equivalent) and would match both that domain AND all subdomains.
  ///
  /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all of the following would match
  /// (and therefore would bypass the proxy):
  /// * `http://google.com/`
  /// * `http://www.google.com/`
  /// * `http://192.168.1.42/`
  ///
  /// The URL `http://notgoogle.com/` would not match.
  fn from_string(no_proxy_list: &str) -> Option<Self> {
    if no_proxy_list.is_empty() {
      return None;
    }
    let mut ips = Vec::new();
    let mut domains = Vec::new();
    let parts = no_proxy_list.split(',').map(str::trim);
    for part in parts {
      match part.parse::<IpNet>() {
        // If we can parse an IP net or address, then use it, otherwise, assume it is a domain
        Ok(ip) => ips.push(Ip::Network(ip)),
        Err(_) => match part.parse::<IpAddr>() {
          Ok(addr) => ips.push(Ip::Address(addr)),
          Err(_) => domains.push(part.to_owned()),
        },
      }
    }
    Some(NoProxy {
      ips: IpMatcher(ips),
      domains: DomainMatcher(domains),
    })
  }

  fn contains(&self, host: &str) -> bool {
    // According to RFC3986, raw IPv6 hosts will be wrapped in []. So we need to strip those off
    // the end in order to parse correctly
    let host = if host.starts_with('[') {
      let x: &[_] = &['[', ']'];
      host.trim_matches(x)
    } else {
      host
    };
    match host.parse::<IpAddr>() {
      // If we can parse an IP addr, then use it, otherwise, assume it is a domain
      Ok(ip) => self.ips.contains(ip),
      Err(_) => self.domains.contains(host),
    }
  }
}

impl IpMatcher {
  fn contains(&self, addr: IpAddr) -> bool {
    for ip in &self.0 {
      match ip {
        Ip::Address(address) => {
          if &addr == address {
            return true;
          }
        }
        Ip::Network(net) => {
          if net.contains(&addr) {
            return true;
          }
        }
      }
    }
    false
  }
}

impl DomainMatcher {
  // The following links may be useful to understand the origin of these rules:
  // * https://curl.se/libcurl/c/CURLOPT_NOPROXY.html
  // * https://github.com/curl/curl/issues/1208
  fn contains(&self, domain: &str) -> bool {
    let domain_len = domain.len();
    for d in &self.0 {
      if d == domain || d.strip_prefix('.') == Some(domain) {
        return true;
      } else if domain.ends_with(d) {
        if d.starts_with('.') {
          // If the first character of d is a dot, that means the first character of domain
          // must also be a dot, so we are looking at a subdomain of d and that matches
          return true;
        } else if domain.as_bytes().get(domain_len - d.len() - 1) == Some(&b'.')
        {
          // Given that d is a prefix of domain, if the prior character in domain is a dot
          // then that means we must be matching a subdomain of d, and that matches
          return true;
        }
      } else if d == "*" {
        return true;
      }
    }
    false
  }
}

impl<C> ProxyConnector<C> {
  fn intercept(&self, dst: &Uri) -> Option<&Intercept> {
    self.proxies.intercept(dst)
  }
}

impl Proxies {
  pub(crate) fn prepend(&mut self, intercept: Intercept) {
    self.intercepts.insert(0, intercept);
  }

  pub(crate) fn http_forward_auth(&self, dst: &Uri) -> Option<&HeaderValue> {
    let intercept = self.intercept(dst)?;
    match intercept.target {
      // Only if the proxy target is http
      Target::Http { ref auth, .. } => auth.as_ref(),
      _ => None,
    }
  }

  fn intercept(&self, dst: &Uri) -> Option<&Intercept> {
    if let Some(no_proxy) = self.no.as_ref()
      && no_proxy.contains(dst.host()?)
    {
      return None;
    }

    for intercept in &self.intercepts {
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
#[allow(clippy::large_enum_variant)]
pub enum Proxied<T> {
  /// Not proxied
  PassThrough(T),
  /// Forwarded via TCP socket
  Tcp(T),
  /// Tunneled through HTTP CONNECT
  HttpTunneled(Box<TokioIo<TlsStream<TokioIo<T>>>>),
  /// Tunneled through SOCKS
  Socks(TokioIo<TcpStream>),
  /// Tunneled through SOCKS and TLS
  SocksTls(TokioIo<TlsStream<TokioIo<TokioIo<TcpStream>>>>),
  /// Forwarded via Unix socket
  #[cfg(not(windows))]
  Unix(TokioIo<UnixStream>),
  /// Forwarded via Vsock socket
  #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
  Vsock(TokioIo<VsockStream>),
}

impl<C> Service<Uri> for ProxyConnector<C>
where
  C: Service<Uri> + Clone,
  C::Response:
    hyper::rt::Read + hyper::rt::Write + Connection + Unpin + Send + 'static,
  C::Future: Send + 'static,
  C::Error: Into<BoxError> + 'static,
{
  type Response = Proxied<MaybeHttpsStream<C::Response>>;
  type Error = BoxError;
  type Future = BoxFuture<Result<Self::Response, Self::Error>>;

  fn poll_ready(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    self.http.poll_ready(cx).map_err(Into::into)
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
          let mut connector =
            HttpsConnector::from((self.http.clone(), self.tls_proxy.clone()));
          let connecting = connector.call(proxy_dst);
          let tls = TlsConnector::from(self.tls.clone());
          Box::pin(async move {
            let mut io = connecting.await?;

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
              Ok(Proxied::Tcp(io))
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
        Target::Tcp {
          hostname: host,
          port,
        } => {
          let mut connector =
            HttpsConnector::from((self.http.clone(), self.tls_proxy.clone()));
          let Ok(uri) = format!("http://{}:{}", host, port).parse() else {
            return Box::pin(async {
              Err("failed to parse tcp proxy uri".into())
            });
          };
          let connecting = connector.call(uri);
          Box::pin(async move {
            let io = connecting.await?;
            Ok(Proxied::Tcp(io))
          })
        }
        #[cfg(not(windows))]
        Target::Unix { path } => {
          let path = path.clone();
          Box::pin(async move {
            let io = UnixStream::connect(&path).await?;
            Ok(Proxied::Unix(TokioIo::new(io)))
          })
        }
        #[cfg(any(
          target_os = "android",
          target_os = "linux",
          target_os = "macos"
        ))]
        Target::Vsock { cid, port } => Box::pin(async move {
          let addr = tokio_vsock::VsockAddr::new(cid, port);
          let io = VsockStream::connect(addr).await?;
          Ok(Proxied::Vsock(TokioIo::new(io)))
        }),
      };
    }

    let mut connector =
      HttpsConnector::from((self.http.clone(), self.tls.clone()));
    Box::pin(
      connector
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

  let mut buf = [0; 8192];
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
      Proxied::Tcp(ref mut p) => Pin::new(p).poll_read(cx, buf),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_read(cx, buf),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_read(cx, buf),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_read(cx, buf),
      #[cfg(not(windows))]
      Proxied::Unix(ref mut p) => Pin::new(p).poll_read(cx, buf),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxied::Vsock(ref mut p) => Pin::new(p).poll_read(cx, buf),
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
      Proxied::Tcp(ref mut p) => Pin::new(p).poll_write(cx, buf),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_write(cx, buf),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_write(cx, buf),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_write(cx, buf),
      #[cfg(not(windows))]
      Proxied::Unix(ref mut p) => Pin::new(p).poll_write(cx, buf),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxied::Vsock(ref mut p) => Pin::new(p).poll_write(cx, buf),
    }
  }

  fn poll_flush(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    match *self {
      Proxied::PassThrough(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::Tcp(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_flush(cx),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_flush(cx),
      #[cfg(not(windows))]
      Proxied::Unix(ref mut p) => Pin::new(p).poll_flush(cx),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxied::Vsock(ref mut p) => Pin::new(p).poll_flush(cx),
    }
  }

  fn poll_shutdown(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    match *self {
      Proxied::PassThrough(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::Tcp(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::HttpTunneled(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::Socks(ref mut p) => Pin::new(p).poll_shutdown(cx),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_shutdown(cx),
      #[cfg(not(windows))]
      Proxied::Unix(ref mut p) => Pin::new(p).poll_shutdown(cx),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxied::Vsock(ref mut p) => Pin::new(p).poll_shutdown(cx),
    }
  }

  fn is_write_vectored(&self) -> bool {
    match *self {
      Proxied::PassThrough(ref p) => p.is_write_vectored(),
      Proxied::Tcp(ref p) => p.is_write_vectored(),
      Proxied::HttpTunneled(ref p) => p.is_write_vectored(),
      Proxied::Socks(ref p) => p.is_write_vectored(),
      Proxied::SocksTls(ref p) => p.is_write_vectored(),
      #[cfg(not(windows))]
      Proxied::Unix(ref p) => p.is_write_vectored(),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxied::Vsock(ref p) => p.is_write_vectored(),
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
      Proxied::Tcp(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
      Proxied::HttpTunneled(ref mut p) => {
        Pin::new(p).poll_write_vectored(cx, bufs)
      }
      Proxied::Socks(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
      Proxied::SocksTls(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
      #[cfg(not(windows))]
      Proxied::Unix(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxied::Vsock(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
    }
  }
}

impl<T> Connection for Proxied<T>
where
  T: Connection,
{
  fn connected(&self) -> Connected {
    match self {
      Proxied::PassThrough(p) => p.connected(),
      Proxied::Tcp(p) => p.connected().proxy(true),
      Proxied::HttpTunneled(p) => {
        let tunneled_tls = p.inner().get_ref();
        if tunneled_tls.1.alpn_protocol() == Some(b"h2") {
          tunneled_tls.0.connected().negotiated_h2()
        } else {
          tunneled_tls.0.connected()
        }
      }
      Proxied::Socks(p) => p.connected(),
      Proxied::SocksTls(p) => {
        let tunneled_tls = p.inner().get_ref();
        if tunneled_tls.1.alpn_protocol() == Some(b"h2") {
          tunneled_tls.0.connected().negotiated_h2()
        } else {
          tunneled_tls.0.connected()
        }
      }
      #[cfg(not(windows))]
      Proxied::Unix(_) => Connected::new().proxy(true),
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxied::Vsock(_) => Connected::new().proxy(true),
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

  // percent encoded user info
  match parse("us%2Fer:p%2Fass@127.0.0.1:6666") {
    Target::Http { dst, auth } => {
      assert_eq!(dst, "http://127.0.0.1:6666");
      let auth = auth.unwrap();
      assert_eq!(auth.to_str().unwrap(), "Basic dXMvZXI6cC9hc3M=");
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

  // unix
  #[cfg(not(windows))]
  match parse("unix:foo%20bar/baz") {
    Target::Unix { path } => {
      assert_eq!(path.to_str(), Some("foo bar/baz"));
    }
    _ => panic!("bad target"),
  }

  // vsock
  #[cfg(any(target_os = "linux", target_os = "macos"))]
  match parse("vsock:1234:5678") {
    Target::Vsock { cid, port } => {
      assert_eq!(cid, 1234);
      assert_eq!(port, 5678);
    }
    _ => panic!("bad target"),
  }
}

#[test]
fn test_domain_matcher() {
  let domains = vec![".foo.bar".into(), "bar.foo".into()];
  let matcher = DomainMatcher(domains);

  // domains match with leading `.`
  assert!(matcher.contains("foo.bar"));
  // subdomains match with leading `.`
  assert!(matcher.contains("www.foo.bar"));

  // domains match with no leading `.`
  assert!(matcher.contains("bar.foo"));
  // subdomains match with no leading `.`
  assert!(matcher.contains("www.bar.foo"));

  // non-subdomain string prefixes don't match
  assert!(!matcher.contains("notfoo.bar"));
  assert!(!matcher.contains("notbar.foo"));
}

#[test]
fn test_no_proxy_wildcard() {
  let no_proxy = NoProxy::from_string("*").unwrap();
  assert!(no_proxy.contains("any.where"));
}

#[test]
fn test_no_proxy_ip_ranges() {
  let no_proxy = NoProxy::from_string(
    ".foo.bar, bar.baz,10.42.1.1/24,::1,10.124.7.8,2001::/17",
  )
  .unwrap();

  let should_not_match = [
    // random url, not in no_proxy
    "deno.com",
    // make sure that random non-subdomain string prefixes don't match
    "notfoo.bar",
    // make sure that random non-subdomain string prefixes don't match
    "notbar.baz",
    // ipv4 address out of range
    "10.43.1.1",
    // ipv4 address out of range
    "10.124.7.7",
    // ipv6 address out of range
    "[ffff:db8:a0b:12f0::1]",
    // ipv6 address out of range
    "[2005:db8:a0b:12f0::1]",
  ];

  for host in &should_not_match {
    assert!(!no_proxy.contains(host), "should not contain {:?}", host);
  }

  let should_match = [
    // make sure subdomains (with leading .) match
    "hello.foo.bar",
    // make sure exact matches (without leading .) match (also makes sure spaces between entries work)
    "bar.baz",
    // make sure subdomains (without leading . in no_proxy) match
    "foo.bar.baz",
    // make sure subdomains (without leading . in no_proxy) match - this differs from cURL
    "foo.bar",
    // ipv4 address match within range
    "10.42.1.100",
    // ipv6 address exact match
    "[::1]",
    // ipv6 address match within range
    "[2001:db8:a0b:12f0::1]",
    // ipv4 address exact match
    "10.124.7.8",
  ];

  for host in &should_match {
    assert!(no_proxy.contains(host), "should contain {:?}", host);
  }
}
