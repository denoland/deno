// Copyright 2018-2026 the Deno authors. MIT license.

//! Hand-rolled implementation of the RFC 6265bis cookie model used by the
//! `Deno.CookieJar`, `Deno.Cookie` and `Deno.CookieMap` APIs as well as the
//! cookie jar support in `Deno.createHttpClient()`.
//!
//! Notable, documented limitations:
//! - No public suffix list. Setting `Domain` to a public suffix (for example
//!   `Domain=co.uk`) is only mitigated by a heuristic that rejects domain
//!   attributes without an embedded dot (unless equal to the request host).
//! - `HttpOnly` cookies are visible through the jar inspection APIs. There is
//!   no `document.cookie` equivalent to hide them from.

use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::url::Url;

/// Total cookies kept in a single store before least-recently-used cookies
/// are evicted. Matches the order of magnitude browsers use.
const MAX_COOKIES: usize = 3000;
/// Cookies kept per domain before least-recently-used eviction.
const MAX_COOKIES_PER_DOMAIN: usize = 180;
/// RFC 6265bis caps persistent cookie lifetimes at 400 days.
const MAX_AGE_CAP_MS: i64 = 400 * 24 * 60 * 60 * 1000;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CookieError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(type)]
  #[error(transparent)]
  Url(#[from] deno_core::url::ParseError),
  #[class(type)]
  #[error("Invalid cookie name")]
  InvalidName,
  #[class(type)]
  #[error("Invalid cookie value")]
  InvalidValue,
  #[class(type)]
  #[error("Invalid cookie {0} attribute")]
  InvalidAttribute(&'static str),
  #[class(type)]
  #[error("Invalid Set-Cookie header")]
  InvalidSetCookie,
  #[class(type)]
  #[error("Cookie rejected: {0}")]
  Rejected(&'static str),
  #[class(type)]
  #[error("A 'url' or a cookie 'domain' is required to store a cookie")]
  DomainRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
  Strict,
  Lax,
  None,
}

impl SameSite {
  pub fn parse(s: &str) -> Option<SameSite> {
    if s.eq_ignore_ascii_case("strict") {
      Some(SameSite::Strict)
    } else if s.eq_ignore_ascii_case("lax") {
      Some(SameSite::Lax)
    } else if s.eq_ignore_ascii_case("none") {
      Some(SameSite::None)
    } else {
      None
    }
  }

  pub fn as_str(&self) -> &'static str {
    match self {
      SameSite::Strict => "Strict",
      SameSite::Lax => "Lax",
      SameSite::None => "None",
    }
  }
}

/// A cookie as parsed from a `Set-Cookie` header (RFC 6265bis section 5.6),
/// before the storage model has been applied.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedCookie {
  pub name: String,
  pub value: String,
  pub domain: Option<String>,
  pub path: Option<String>,
  /// Parsed `Expires` attribute as milliseconds since the epoch.
  pub expires_ms: Option<i64>,
  /// `Max-Age` attribute in seconds.
  pub max_age: Option<i64>,
  pub secure: bool,
  pub http_only: bool,
  pub same_site: Option<SameSite>,
  pub partitioned: bool,
}

/// A cookie in the store after the RFC 6265bis section 5.7 storage model has
/// been applied.
#[derive(Debug, Clone)]
pub struct StoredCookie {
  pub name: String,
  pub value: String,
  /// Canonical lowercase domain without a leading dot.
  pub domain: String,
  pub path: String,
  pub host_only: bool,
  pub secure: bool,
  pub http_only: bool,
  pub same_site: Option<SameSite>,
  pub partitioned: bool,
  /// `None` means a session cookie.
  pub expiry_ms: Option<i64>,
  pub creation_time_ms: i64,
  pub creation_index: u64,
  pub last_access_ms: i64,
}

impl StoredCookie {
  fn expired(&self, now_ms: i64) -> bool {
    matches!(self.expiry_ms, Some(expiry) if expiry <= now_ms)
  }
}

#[derive(Debug, Default)]
pub struct CookieStore {
  cookies: Vec<StoredCookie>,
  next_index: u64,
}

pub fn now_ms() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}

fn is_ows(c: char) -> bool {
  c == ' ' || c == '\t'
}

/// RFC 2616 token characters, used for cookie names.
fn is_token_char(c: char) -> bool {
  matches!(c, '!' | '#'..='\'' | '*' | '+' | '-' | '.' | '0'..='9'
    | 'A'..='Z' | '^'..='z' | '|' | '~')
}

/// RFC 6265 `cookie-octet`.
fn is_cookie_octet(c: char) -> bool {
  matches!(c, '\x21' | '\x23'..='\x2B' | '\x2D'..='\x3A' | '\x3C'..='\x5B'
    | '\x5D'..='\x7E')
}

pub fn valid_cookie_name(name: &str) -> bool {
  !name.is_empty() && name.chars().all(is_token_char)
}

pub fn valid_cookie_value(value: &str) -> bool {
  value.chars().all(is_cookie_octet)
}

fn is_loopback_host(host: &str) -> bool {
  if host == "localhost" || host.ends_with(".localhost") {
    return true;
  }
  if let Ok(ip) = host.parse::<Ipv4Addr>() {
    return ip.is_loopback();
  }
  if let Some(stripped) = host.strip_prefix('[')
    && let Some(stripped) = stripped.strip_suffix(']')
    && let Ok(ip) = stripped.parse::<Ipv6Addr>()
  {
    return ip.is_loopback();
  }
  false
}

/// Whether cookies marked `Secure` may be set from and sent to this URL.
/// Secure schemes and loopback hosts are considered trustworthy, matching
/// browser behavior.
fn url_is_trustworthy(url: &Url) -> bool {
  matches!(url.scheme(), "https" | "wss")
    || url.host_str().map(is_loopback_host).unwrap_or(false)
}

fn is_ip_host(host: &str) -> bool {
  host.parse::<Ipv4Addr>().is_ok()
    || (host.starts_with('[') && host.ends_with(']'))
}

/// Canonicalizes a domain attribute value: lowercases and applies IDNA by
/// round-tripping through URL parsing (RFC 6265bis section 5.1.2).
fn canonicalize_domain(domain: &str) -> Option<String> {
  if domain.is_empty()
    || domain.contains(['/', '\\', '@', ':', '?', '#'])
    || domain.starts_with('[')
  {
    return None;
  }
  let url = Url::parse(&format!("http://{domain}/")).ok()?;
  url.host_str().map(|h| h.to_string())
}

/// RFC 6265bis section 5.1.3.
fn domain_matches(host: &str, domain: &str) -> bool {
  if host == domain {
    return true;
  }
  host.ends_with(domain)
    && host[..host.len() - domain.len()].ends_with('.')
    && !is_ip_host(host)
}

/// RFC 6265bis section 5.1.4: computes the default path of a request URL.
fn default_path(url: &Url) -> String {
  let path = url.path();
  if !path.starts_with('/') {
    return "/".to_string();
  }
  match path.rfind('/') {
    Some(0) | None => "/".to_string(),
    Some(i) => path[..i].to_string(),
  }
}

/// RFC 6265bis section 5.1.4 path matching.
fn path_matches(request_path: &str, cookie_path: &str) -> bool {
  if request_path == cookie_path {
    return true;
  }
  request_path.starts_with(cookie_path)
    && (cookie_path.ends_with('/')
      || request_path.as_bytes().get(cookie_path.len()) == Some(&b'/'))
}

fn request_path(url: &Url) -> &str {
  let path = url.path();
  if path.is_empty() { "/" } else { path }
}

const fn is_delimiter(c: char) -> bool {
  matches!(c, '\x09' | '\x20'..='\x2F' | '\x3B'..='\x40' | '\x5B'..='\x60'
    | '\x7B'..='\x7E')
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
  let y = if month <= 2 { year - 1 } else { year };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let mp = (month as i64 + 9) % 12;
  let doy = (153 * mp + 2) / 5 + day as i64 - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146097 + doe - 719468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
  let z = days + 719468;
  let era = if z >= 0 { z } else { z - 146096 } / 146097;
  let doe = z - era * 146097;
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
  let y = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
  let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
  (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Parses a cookie date per the RFC 6265bis section 5.5 algorithm. Returns
/// milliseconds since the epoch.
pub fn parse_cookie_date(s: &str) -> Option<i64> {
  let mut hms: Option<(u32, u32, u32)> = None;
  let mut day: Option<u32> = None;
  let mut month: Option<u32> = None;
  let mut year: Option<i64> = None;

  const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct",
    "nov", "dec",
  ];

  fn leading_digits(token: &str, min: usize, max: usize) -> Option<u64> {
    let digits: String =
      token.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < min || digits.len() > max {
      return None;
    }
    digits.parse().ok()
  }

  fn parse_time(token: &str) -> Option<(u32, u32, u32)> {
    let mut parts = token.splitn(3, ':');
    let h = leading_digits(parts.next()?, 1, 2)?;
    let m = leading_digits(parts.next()?, 1, 2)?;
    // The seconds field may have trailing non-digits per the grammar.
    let s = leading_digits(parts.next()?, 1, 2)?;
    Some((h as u32, m as u32, s as u32))
  }

  for token in s.split(is_delimiter).filter(|t| !t.is_empty()) {
    if hms.is_none()
      && token.matches(':').count() >= 2
      && let Some(t) = parse_time(token)
    {
      hms = Some(t);
      continue;
    }
    if day.is_none()
      && let Some(d) = leading_digits(token, 1, 2)
    {
      day = Some(d as u32);
      continue;
    }
    if month.is_none() && token.len() >= 3 && token.is_char_boundary(3) {
      let prefix = token[..3].to_ascii_lowercase();
      if let Some(idx) = MONTHS.iter().position(|m| **m == prefix) {
        month = Some(idx as u32 + 1);
        continue;
      }
    }
    if year.is_none()
      && let Some(y) = leading_digits(token, 2, 4)
    {
      year = Some(y as i64);
      continue;
    }
  }

  let (hour, minute, second) = hms?;
  let day = day?;
  let month = month?;
  let mut year = year?;

  if (70..=99).contains(&year) {
    year += 1900;
  } else if (0..=69).contains(&year) {
    year += 2000;
  }

  if !(1..=31).contains(&day)
    || year < 1601
    || hour > 23
    || minute > 59
    || second > 59
  {
    return None;
  }

  let days = days_from_civil(year, month, day);
  (days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64)
    .checked_mul(1000)
}

/// Formats milliseconds since the epoch as an IMF-fixdate
/// (e.g. `Sun, 06 Nov 1994 08:49:37 GMT`).
pub fn format_http_date(ms: i64) -> String {
  const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
  const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
    "Nov", "Dec",
  ];
  let secs = ms.div_euclid(1000);
  let days = secs.div_euclid(86400);
  let time = secs.rem_euclid(86400);
  let (year, month, day) = civil_from_days(days);
  // 1970-01-01 was a Thursday.
  let weekday = WEEKDAYS[days.rem_euclid(7) as usize];
  format!(
    "{weekday}, {day:02} {} {year:04} {:02}:{:02}:{:02} GMT",
    MONTHS[(month - 1) as usize],
    time / 3600,
    (time % 3600) / 60,
    time % 60,
  )
}

/// Parses a `Set-Cookie` header value per RFC 6265bis section 5.6. Returns
/// `None` when the cookie must be ignored entirely.
pub fn parse_set_cookie(input: &str) -> Option<ParsedCookie> {
  // Abort on control characters (HTAB excepted).
  if input
    .chars()
    .any(|c| matches!(c, '\x00'..='\x08' | '\x0A'..='\x1F' | '\x7F'))
  {
    return None;
  }

  let (pair, attributes) = match input.find(';') {
    Some(i) => (&input[..i], &input[i + 1..]),
    None => (input, ""),
  };

  let eq = pair.find('=')?;
  let name = pair[..eq].trim_matches(is_ows);
  let value = pair[eq + 1..].trim_matches(is_ows);
  if name.is_empty() && value.is_empty() {
    return None;
  }
  if name.len() + value.len() > 4096 {
    return None;
  }

  let mut cookie = ParsedCookie {
    name: name.to_string(),
    value: value.to_string(),
    ..Default::default()
  };

  for attr in attributes.split(';') {
    let (attr_name, attr_value) = match attr.find('=') {
      Some(i) => (&attr[..i], &attr[i + 1..]),
      None => (attr, ""),
    };
    let attr_name = attr_name.trim_matches(is_ows);
    let attr_value = attr_value.trim_matches(is_ows);
    if attr_value.len() > 1024 {
      continue;
    }
    if attr_name.eq_ignore_ascii_case("expires") {
      if let Some(ms) = parse_cookie_date(attr_value) {
        cookie.expires_ms = Some(ms);
      }
    } else if attr_name.eq_ignore_ascii_case("max-age") {
      let mut chars = attr_value.chars();
      let valid = match chars.next() {
        Some(c) if c.is_ascii_digit() || c == '-' => {
          chars.all(|c| c.is_ascii_digit()) && attr_value != "-"
        }
        _ => false,
      };
      if valid {
        // Saturate on overflow instead of ignoring the attribute.
        let max_age =
          attr_value
            .parse::<i64>()
            .unwrap_or(if attr_value.starts_with('-') {
              i64::MIN
            } else {
              i64::MAX
            });
        cookie.max_age = Some(max_age);
      }
    } else if attr_name.eq_ignore_ascii_case("domain") {
      if !attr_value.is_empty() {
        let domain = attr_value.strip_prefix('.').unwrap_or(attr_value);
        cookie.domain = Some(domain.to_ascii_lowercase());
      }
    } else if attr_name.eq_ignore_ascii_case("path") {
      cookie.path = Some(attr_value.to_string());
    } else if attr_name.eq_ignore_ascii_case("secure") {
      cookie.secure = true;
    } else if attr_name.eq_ignore_ascii_case("httponly") {
      cookie.http_only = true;
    } else if attr_name.eq_ignore_ascii_case("samesite") {
      cookie.same_site = SameSite::parse(attr_value);
    } else if attr_name.eq_ignore_ascii_case("partitioned") {
      cookie.partitioned = true;
    }
  }

  Some(cookie)
}

/// Serializes a cookie into a `Set-Cookie` header value, validating the
/// name, value and attributes.
pub fn serialize_set_cookie(
  cookie: &ParsedCookie,
) -> Result<String, CookieError> {
  if !valid_cookie_name(&cookie.name) {
    return Err(CookieError::InvalidName);
  }
  if !valid_cookie_value(&cookie.value) {
    return Err(CookieError::InvalidValue);
  }
  let mut out = format!("{}={}", cookie.name, cookie.value);
  if let Some(domain) = &cookie.domain {
    if canonicalize_domain(domain).is_none() || domain.contains(';') {
      return Err(CookieError::InvalidAttribute("domain"));
    }
    out.push_str("; Domain=");
    out.push_str(domain);
  }
  if let Some(path) = &cookie.path {
    if !path.starts_with('/')
      || path.contains(';')
      || !path.chars().all(|c| ('\x20'..='\x7E').contains(&c))
    {
      return Err(CookieError::InvalidAttribute("path"));
    }
    out.push_str("; Path=");
    out.push_str(path);
  }
  if let Some(expires_ms) = cookie.expires_ms {
    out.push_str("; Expires=");
    out.push_str(&format_http_date(expires_ms));
  }
  if let Some(max_age) = cookie.max_age {
    out.push_str("; Max-Age=");
    out.push_str(&max_age.to_string());
  }
  if cookie.secure {
    out.push_str("; Secure");
  }
  if cookie.http_only {
    out.push_str("; HttpOnly");
  }
  if let Some(same_site) = cookie.same_site {
    out.push_str("; SameSite=");
    out.push_str(same_site.as_str());
  }
  if cookie.partitioned {
    out.push_str("; Partitioned");
  }
  Ok(out)
}

/// Leniently parses a `Cookie` request header into name/value pairs.
/// Segments without `=` are skipped.
pub fn parse_cookie_header(input: &str) -> Vec<(String, String)> {
  let mut out = Vec::new();
  for segment in input.split(';') {
    let segment = segment.trim_matches(is_ows);
    if segment.is_empty() {
      continue;
    }
    let Some(eq) = segment.find('=') else {
      continue;
    };
    let name = segment[..eq].trim_matches(is_ows);
    let value = segment[eq + 1..].trim_matches(is_ows);
    out.push((name.to_string(), value.to_string()));
  }
  out
}

impl CookieStore {
  /// Applies the RFC 6265bis section 5.7 storage model to a cookie received
  /// in a response from `url`.
  pub fn store_response_cookie(
    &mut self,
    parsed: ParsedCookie,
    url: &Url,
    now_ms: i64,
  ) -> Result<(), CookieError> {
    let Some(host) = url.host_str() else {
      return Err(CookieError::Rejected("request URL has no host"));
    };
    let host = host.to_ascii_lowercase();

    // Expiry: Max-Age takes precedence over Expires.
    let expiry_ms = if let Some(max_age) = parsed.max_age {
      Some(now_ms.saturating_add(max_age.saturating_mul(1000)))
    } else {
      parsed.expires_ms
    };
    // Cap persistent cookie lifetimes at 400 days.
    let expiry_ms =
      expiry_ms.map(|e| e.min(now_ms.saturating_add(MAX_AGE_CAP_MS)));

    // Domain attribute handling.
    let (domain, host_only) = match &parsed.domain {
      Some(domain_attr) => {
        let Some(domain) = canonicalize_domain(domain_attr) else {
          return Err(CookieError::Rejected("invalid domain attribute"));
        };
        if is_ip_host(&host) {
          if domain != host {
            return Err(CookieError::Rejected(
              "domain attribute does not match IP address host",
            ));
          }
          (host.clone(), true)
        } else {
          if !domain_matches(&host, &domain) {
            return Err(CookieError::Rejected(
              "domain attribute does not domain-match the request host",
            ));
          }
          // Public suffix list heuristic: reject TLD-wide cookies.
          if !domain.contains('.') && domain != host {
            return Err(CookieError::Rejected("domain attribute is too broad"));
          }
          (domain, false)
        }
      }
      None => (host.clone(), true),
    };

    let path = match &parsed.path {
      Some(p) if p.starts_with('/') => p.clone(),
      _ => default_path(url),
    };

    let trustworthy = url_is_trustworthy(url);
    if parsed.secure && !trustworthy {
      return Err(CookieError::Rejected(
        "secure cookie set over an insecure connection",
      ));
    }

    // Cookie name prefixes (case-insensitive per RFC 6265bis).
    let lower_name = parsed.name.to_ascii_lowercase();
    if lower_name.starts_with("__secure-") && !parsed.secure {
      return Err(CookieError::Rejected(
        "__Secure- cookie without Secure attribute",
      ));
    }
    if lower_name.starts_with("__host-")
      && (!parsed.secure
        || parsed.domain.is_some()
        || parsed.path.as_deref() != Some("/"))
    {
      return Err(CookieError::Rejected(
        "__Host- cookie requires Secure, no Domain, and Path=/",
      ));
    }

    // A non-secure connection may not overwrite or shadow a secure cookie
    // ("secure cookie shadowing", RFC 6265bis section 5.7).
    if !trustworthy {
      let shadowed = self.cookies.iter().any(|c| {
        c.secure
          && c.name == parsed.name
          && (domain_matches(&domain, &c.domain)
            || domain_matches(&c.domain, &domain))
          && path_matches(&path, &c.path)
      });
      if shadowed {
        return Err(CookieError::Rejected(
          "cannot shadow a secure cookie from an insecure connection",
        ));
      }
    }

    self.insert(StoredCookie {
      name: parsed.name,
      value: parsed.value,
      domain,
      path,
      host_only,
      secure: parsed.secure,
      http_only: parsed.http_only,
      same_site: parsed.same_site,
      partitioned: parsed.partitioned,
      expiry_ms,
      creation_time_ms: now_ms,
      creation_index: 0,
      last_access_ms: now_ms,
    });
    Ok(())
  }

  /// Stores a cookie that carries an explicit domain, without a request URL
  /// (used by `CookieJar.prototype.setCookie` when no URL is provided).
  pub fn store_explicit_cookie(
    &mut self,
    parsed: ParsedCookie,
    now_ms: i64,
  ) -> Result<(), CookieError> {
    let Some(domain_attr) = &parsed.domain else {
      return Err(CookieError::DomainRequired);
    };
    let Some(domain) = canonicalize_domain(domain_attr) else {
      return Err(CookieError::Rejected("invalid domain attribute"));
    };
    let expiry_ms = if let Some(max_age) = parsed.max_age {
      Some(now_ms.saturating_add(max_age.saturating_mul(1000)))
    } else {
      parsed.expires_ms
    };
    let expiry_ms =
      expiry_ms.map(|e| e.min(now_ms.saturating_add(MAX_AGE_CAP_MS)));
    let path = match &parsed.path {
      Some(p) if p.starts_with('/') => p.clone(),
      _ => "/".to_string(),
    };
    self.insert(StoredCookie {
      name: parsed.name,
      value: parsed.value,
      host_only: is_ip_host(&domain),
      domain,
      path,
      secure: parsed.secure,
      http_only: parsed.http_only,
      same_site: parsed.same_site,
      partitioned: parsed.partitioned,
      expiry_ms,
      creation_time_ms: now_ms,
      creation_index: 0,
      last_access_ms: now_ms,
    });
    Ok(())
  }

  fn insert(&mut self, mut cookie: StoredCookie) {
    self.next_index += 1;
    cookie.creation_index = self.next_index;

    // Replacement: a cookie with the same name, domain and path keeps the
    // creation time of the cookie it replaces.
    if let Some(pos) = self.cookies.iter().position(|c| {
      c.name == cookie.name
        && c.domain == cookie.domain
        && c.path == cookie.path
    }) {
      let old = self.cookies.remove(pos);
      cookie.creation_time_ms = old.creation_time_ms;
      cookie.creation_index = old.creation_index;
    }

    // An already-expired cookie acts as a deletion.
    if cookie.expired(cookie.last_access_ms) {
      return;
    }

    self.cookies.push(cookie);
    self.evict();
  }

  fn evict(&mut self) {
    let domain = self.cookies.last().map(|c| c.domain.clone());
    if let Some(domain) = domain {
      let per_domain =
        self.cookies.iter().filter(|c| c.domain == domain).count();
      if per_domain > MAX_COOKIES_PER_DOMAIN {
        self.evict_least_recently_used(Some(&domain));
      }
    }
    if self.cookies.len() > MAX_COOKIES {
      self.evict_least_recently_used(None);
    }
  }

  fn evict_least_recently_used(&mut self, domain: Option<&str>) {
    let pos = self
      .cookies
      .iter()
      .enumerate()
      .filter(|(_, c)| domain.is_none_or(|d| c.domain == d))
      .min_by_key(|(_, c)| (c.last_access_ms, c.creation_index))
      .map(|(i, _)| i);
    if let Some(pos) = pos {
      self.cookies.remove(pos);
    }
  }

  fn purge_expired(&mut self, now_ms: i64) {
    self.cookies.retain(|c| !c.expired(now_ms));
  }

  /// RFC 6265bis section 5.8.3: indices of cookies to include in a request
  /// to `url`, sorted by path length (longest first) and creation time.
  fn matching_indices(&mut self, url: &Url, now_ms: i64) -> Vec<usize> {
    self.purge_expired(now_ms);
    let Some(host) = url.host_str().map(|h| h.to_ascii_lowercase()) else {
      return Vec::new();
    };
    let trustworthy = url_is_trustworthy(url);
    let path = request_path(url);
    let mut indices: Vec<usize> = self
      .cookies
      .iter()
      .enumerate()
      .filter(|(_, c)| {
        let domain_ok = if c.host_only {
          host == c.domain
        } else {
          domain_matches(&host, &c.domain)
        };
        domain_ok && path_matches(path, &c.path) && (!c.secure || trustworthy)
      })
      .map(|(i, _)| i)
      .collect();
    indices.sort_by(|&a, &b| {
      let ca = &self.cookies[a];
      let cb = &self.cookies[b];
      cb.path
        .len()
        .cmp(&ca.path.len())
        .then(ca.creation_index.cmp(&cb.creation_index))
    });
    for &i in &indices {
      self.cookies[i].last_access_ms = now_ms;
    }
    indices
  }

  /// Computes the `Cookie` request header value for a request to `url`, or
  /// `None` when no cookies match.
  pub fn cookie_header(&mut self, url: &Url, now_ms: i64) -> Option<String> {
    let indices = self.matching_indices(url, now_ms);
    if indices.is_empty() {
      return None;
    }
    let mut out = String::new();
    for i in indices {
      let cookie = &self.cookies[i];
      if !out.is_empty() {
        out.push_str("; ");
      }
      if cookie.name.is_empty() {
        out.push_str(&cookie.value);
      } else {
        out.push_str(&cookie.name);
        out.push('=');
        out.push_str(&cookie.value);
      }
    }
    Some(out)
  }

  /// Returns the cookies that would be sent to `url`, with attributes.
  pub fn get_cookies(&mut self, url: &Url, now_ms: i64) -> Vec<StoredCookie> {
    self
      .matching_indices(url, now_ms)
      .into_iter()
      .map(|i| self.cookies[i].clone())
      .collect()
  }

  /// Returns all unexpired cookies in the store.
  pub fn entries(&mut self, now_ms: i64) -> Vec<StoredCookie> {
    self.purge_expired(now_ms);
    let mut cookies = self.cookies.clone();
    cookies.sort_by_key(|c| c.creation_index);
    cookies
  }

  /// Deletes cookies by name, optionally constrained to a domain and path.
  /// Returns the number of cookies removed.
  pub fn delete(
    &mut self,
    name: &str,
    domain: Option<&str>,
    path: Option<&str>,
  ) -> usize {
    let domain =
      domain.map(|d| d.strip_prefix('.').unwrap_or(d).to_ascii_lowercase());
    let before = self.cookies.len();
    self.cookies.retain(|c| {
      !(c.name == name
        && domain.as_deref().is_none_or(|d| c.domain == d)
        && path.is_none_or(|p| c.path == p))
    });
    before - self.cookies.len()
  }

  pub fn clear(&mut self) {
    self.cookies.clear();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const NOW: i64 = 1_700_000_000_000;

  fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
  }

  fn store_from(responses: &[(&str, &str)]) -> CookieStore {
    let mut store = CookieStore::default();
    for (i, (u, set_cookie)) in responses.iter().enumerate() {
      let parsed = parse_set_cookie(set_cookie).expect(set_cookie);
      // Space creation times one ms apart to make ordering deterministic.
      let _ = store.store_response_cookie(parsed, &url(u), NOW + i as i64);
    }
    store
  }

  #[test]
  fn parse_basic() {
    let c = parse_set_cookie("foo=bar").unwrap();
    assert_eq!(c.name, "foo");
    assert_eq!(c.value, "bar");
    assert!(!c.secure);
    assert_eq!(c.domain, None);

    let c = parse_set_cookie(" foo = bar ; Path = /sub ; Secure ").unwrap();
    assert_eq!(c.name, "foo");
    assert_eq!(c.value, "bar");
    assert_eq!(c.path.as_deref(), Some("/sub"));
    assert!(c.secure);

    // Value with '=' inside.
    let c = parse_set_cookie("foo=bar=baz").unwrap();
    assert_eq!(c.value, "bar=baz");

    // Nameless cookie.
    let c = parse_set_cookie("=bar").unwrap();
    assert_eq!(c.name, "");
    assert_eq!(c.value, "bar");
  }

  #[test]
  fn parse_rejections() {
    // No '=' in the name-value pair.
    assert_eq!(parse_set_cookie("foo"), None);
    // Empty name and value.
    assert_eq!(parse_set_cookie("="), None);
    assert_eq!(parse_set_cookie(""), None);
    // Control characters.
    assert_eq!(parse_set_cookie("foo=b\x01ar"), None);
    assert_eq!(parse_set_cookie("foo=bar\x0a; Path=/"), None);
    // Oversized name+value.
    let big = format!("foo={}", "x".repeat(4096));
    assert_eq!(parse_set_cookie(&big), None);
  }

  #[test]
  fn parse_attributes() {
    let c = parse_set_cookie(
      "a=b; dOmAiN=.Example.COM; HTTPONLY; SameSite=lax; Max-Age=100; Partitioned",
    )
    .unwrap();
    assert_eq!(c.domain.as_deref(), Some("example.com"));
    assert!(c.http_only);
    assert_eq!(c.same_site, Some(SameSite::Lax));
    assert_eq!(c.max_age, Some(100));
    assert!(c.partitioned);

    // Unknown SameSite value is treated as unset.
    let c = parse_set_cookie("a=b; SameSite=bogus").unwrap();
    assert_eq!(c.same_site, None);

    // Invalid Max-Age values are ignored.
    assert_eq!(parse_set_cookie("a=b; Max-Age=12x").unwrap().max_age, None);
    assert_eq!(parse_set_cookie("a=b; Max-Age=x").unwrap().max_age, None);
    assert_eq!(parse_set_cookie("a=b; Max-Age=-").unwrap().max_age, None);
    assert_eq!(
      parse_set_cookie("a=b; Max-Age=-5").unwrap().max_age,
      Some(-5)
    );

    // Oversized attribute values are ignored, cookie is kept.
    let big = format!("a=b; Path=/{}", "x".repeat(1024));
    let c = parse_set_cookie(&big).unwrap();
    assert_eq!(c.path, None);

    // Max-Age overflow saturates.
    let c = parse_set_cookie("a=b; Max-Age=99999999999999999999").unwrap();
    assert_eq!(c.max_age, Some(i64::MAX));
  }

  #[test]
  fn parse_dates() {
    // RFC 1123 format.
    assert_eq!(
      parse_cookie_date("Sun, 06 Nov 1994 08:49:37 GMT"),
      Some(784111777000)
    );
    // RFC 850 format.
    assert_eq!(
      parse_cookie_date("Sunday, 06-Nov-94 08:49:37 GMT"),
      Some(784111777000)
    );
    // asctime format.
    assert_eq!(
      parse_cookie_date("Sun Nov  6 08:49:37 1994"),
      Some(784111777000)
    );
    // Two-digit year fixups.
    assert_eq!(
      parse_cookie_date("06 Nov 70 08:49:37"),
      parse_cookie_date("06 Nov 1970 08:49:37")
    );
    assert_eq!(
      parse_cookie_date("06 Nov 69 08:49:37"),
      parse_cookie_date("06 Nov 2069 08:49:37")
    );
    // Rejections.
    assert_eq!(parse_cookie_date(""), None);
    assert_eq!(parse_cookie_date("garbage"), None);
    assert_eq!(parse_cookie_date("32 Nov 1994 08:49:37"), None);
    assert_eq!(parse_cookie_date("06 Nov 1600 08:49:37"), None);
    assert_eq!(parse_cookie_date("06 Nov 1994 24:49:37"), None);
    assert_eq!(parse_cookie_date("06 Nov 1994"), None);
  }

  #[test]
  fn http_date_round_trip() {
    let formatted = format_http_date(784111777000);
    assert_eq!(formatted, "Sun, 06 Nov 1994 08:49:37 GMT");
    assert_eq!(parse_cookie_date(&formatted), Some(784111777000));
    assert_eq!(format_http_date(0), "Thu, 01 Jan 1970 00:00:00 GMT");
  }

  #[test]
  fn domain_and_path_matching() {
    assert!(domain_matches("example.com", "example.com"));
    assert!(domain_matches("www.example.com", "example.com"));
    assert!(!domain_matches("example.com", "www.example.com"));
    assert!(!domain_matches("badexample.com", "example.com"));
    assert!(!domain_matches("127.0.0.1", "0.0.1"));

    assert!(path_matches("/", "/"));
    assert!(path_matches("/foo/bar", "/foo"));
    assert!(path_matches("/foo/bar", "/foo/"));
    assert!(!path_matches("/foobar", "/foo"));
    assert!(!path_matches("/foo", "/foo/bar"));

    assert_eq!(default_path(&url("http://a.com")), "/");
    assert_eq!(default_path(&url("http://a.com/foo")), "/");
    assert_eq!(default_path(&url("http://a.com/foo/bar")), "/foo");
    assert_eq!(default_path(&url("http://a.com/foo/bar/")), "/foo/bar");
  }

  #[test]
  fn storage_host_only_and_domain() {
    let mut store = store_from(&[
      ("http://example.com/", "host=1"),
      ("http://example.com/", "wide=1; Domain=example.com"),
    ]);
    // Host-only cookie is not sent to subdomains; domain cookie is.
    assert_eq!(
      store.cookie_header(&url("http://example.com/"), NOW),
      Some("host=1; wide=1".to_string())
    );
    assert_eq!(
      store.cookie_header(&url("http://www.example.com/"), NOW),
      Some("wide=1".to_string())
    );
    // Unrelated host gets nothing.
    assert_eq!(store.cookie_header(&url("http://other.com/"), NOW), None);
  }

  #[test]
  fn storage_domain_rejections() {
    let mut store = CookieStore::default();
    // Domain not matching the request host.
    let c = parse_set_cookie("a=b; Domain=other.com").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://example.com/"), NOW)
        .is_err()
    );
    // Domain broader than the host (sub setting parent is fine, but a
    // TLD-wide domain with no dot is rejected).
    let c = parse_set_cookie("a=b; Domain=com").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://example.com/"), NOW)
        .is_err()
    );
    // IP hosts only accept an exact domain match, stored host-only.
    let c = parse_set_cookie("a=b; Domain=127.0.0.1").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://127.0.0.1/"), NOW)
        .is_ok()
    );
    let c = parse_set_cookie("a=b; Domain=0.0.1").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://127.0.0.1/"), NOW)
        .is_err()
    );
  }

  #[test]
  fn storage_secure_rules() {
    let mut store = CookieStore::default();
    // Secure cookie over plain http to a non-loopback host: rejected.
    let c = parse_set_cookie("a=b; Secure").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://example.com/"), NOW)
        .is_err()
    );
    // Allowed over https and over http to localhost.
    let c = parse_set_cookie("a=b; Secure").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("https://example.com/"), NOW)
        .is_ok()
    );
    let c = parse_set_cookie("a=b; Secure").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://localhost:8080/"), NOW)
        .is_ok()
    );
    // Secure cookies are not sent over insecure connections.
    assert_eq!(store.cookie_header(&url("http://example.com/"), NOW), None);
    assert_eq!(
      store.cookie_header(&url("https://example.com/"), NOW),
      Some("a=b".to_string())
    );
  }

  #[test]
  fn storage_secure_shadowing() {
    let mut store =
      store_from(&[("https://example.com/", "session=secret; Secure; Path=/")]);
    // An insecure response may not overwrite or shadow the secure cookie.
    let c = parse_set_cookie("session=evil; Path=/login").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://example.com/"), NOW)
        .is_err()
    );
    // A different name is fine.
    let c = parse_set_cookie("other=1").unwrap();
    assert!(
      store
        .store_response_cookie(c, &url("http://example.com/"), NOW)
        .is_ok()
    );
  }

  #[test]
  fn storage_prefixes() {
    let mut store = CookieStore::default();
    let https = url("https://example.com/");

    let ok = parse_set_cookie("__Host-a=b; Secure; Path=/").unwrap();
    assert!(store.store_response_cookie(ok, &https, NOW).is_ok());

    for bad in [
      "__Host-a=b; Path=/",            // not secure
      "__Host-a=b; Secure",            // no Path=/
      "__Host-a=b; Secure; Path=/sub", // wrong path
      "__Host-a=b; Secure; Path=/; Domain=example.com", // has domain
      "__Secure-a=b",                  // not secure
      "__secure-a=b",                  // case-insensitive
    ] {
      let c = parse_set_cookie(bad).unwrap();
      assert!(
        store.store_response_cookie(c, &https, NOW).is_err(),
        "{bad}"
      );
    }

    let ok = parse_set_cookie("__Secure-a=b; Secure").unwrap();
    assert!(store.store_response_cookie(ok, &https, NOW).is_ok());
  }

  #[test]
  fn storage_replacement_preserves_creation_time() {
    let mut store = store_from(&[
      ("http://example.com/", "a=1; Path=/"),
      ("http://example.com/", "b=2; Path=/"),
    ]);
    // Replace "a" later; it must keep its original creation order.
    let c = parse_set_cookie("a=3; Path=/").unwrap();
    store
      .store_response_cookie(c, &url("http://example.com/"), NOW + 100)
      .unwrap();
    assert_eq!(
      store.cookie_header(&url("http://example.com/"), NOW + 200),
      Some("a=3; b=2".to_string())
    );
  }

  #[test]
  fn storage_expiry() {
    let mut store = store_from(&[
      ("http://example.com/", "keep=1; Max-Age=1000"),
      ("http://example.com/", "gone=1; Max-Age=1"),
      ("http://example.com/", "session=1"),
    ]);
    let u = url("http://example.com/");
    assert_eq!(
      store.cookie_header(&u, NOW + 100),
      Some("keep=1; gone=1; session=1".to_string())
    );
    // After "gone" expires it is purged; session cookies stay.
    assert_eq!(
      store.cookie_header(&u, NOW + 10_000),
      Some("keep=1; session=1".to_string())
    );
    // Max-Age has precedence over Expires.
    let c = parse_set_cookie(
      "keep=1; Max-Age=0; Expires=Wed, 01 Jan 2120 00:00:00 GMT",
    )
    .unwrap();
    store.store_response_cookie(c, &u, NOW + 10_000).unwrap();
    assert_eq!(
      store.cookie_header(&u, NOW + 10_001),
      Some("session=1".to_string())
    );
    // 400-day cap.
    let c = parse_set_cookie("a=b; Max-Age=99999999999").unwrap();
    store.store_response_cookie(c, &u, NOW).unwrap();
    let entries = store.entries(NOW);
    let a = entries.iter().find(|c| c.name == "a").unwrap();
    assert_eq!(a.expiry_ms, Some(NOW + MAX_AGE_CAP_MS));
  }

  #[test]
  fn request_ordering() {
    // Longer paths first, then earlier creation.
    let mut store = store_from(&[
      ("http://example.com/foo/bar/", "later=1; Path=/"),
      ("http://example.com/foo/bar/", "deep=1; Path=/foo/bar"),
      ("http://example.com/foo/bar/", "mid=1; Path=/foo"),
    ]);
    assert_eq!(
      store.cookie_header(&url("http://example.com/foo/bar/baz"), NOW + 10),
      Some("deep=1; mid=1; later=1".to_string())
    );
    // Path scoping excludes non-matching paths.
    assert_eq!(
      store.cookie_header(&url("http://example.com/other"), NOW + 10),
      Some("later=1".to_string())
    );
  }

  #[test]
  fn delete_and_clear() {
    let mut store = store_from(&[
      ("http://example.com/", "a=1"),
      ("http://example.com/", "a=2; Path=/sub"),
      ("http://www.example.com/", "a=3"),
      ("http://example.com/", "b=1"),
    ]);
    assert_eq!(store.delete("a", Some("example.com"), Some("/sub")), 1);
    assert_eq!(store.delete("a", Some("example.com"), None), 1);
    assert_eq!(store.delete("a", None, None), 1);
    assert_eq!(store.delete("nope", None, None), 0);
    store.clear();
    assert_eq!(store.entries(NOW).len(), 0);
  }

  #[test]
  fn explicit_store() {
    let mut store = CookieStore::default();
    let c = ParsedCookie {
      name: "a".to_string(),
      value: "b".to_string(),
      domain: Some("example.com".to_string()),
      ..Default::default()
    };
    store.store_explicit_cookie(c, NOW).unwrap();
    assert_eq!(
      store.cookie_header(&url("http://www.example.com/"), NOW),
      Some("a=b".to_string())
    );
    // Domain is required.
    let c = ParsedCookie {
      name: "a".to_string(),
      value: "b".to_string(),
      ..Default::default()
    };
    assert!(matches!(
      store.store_explicit_cookie(c, NOW),
      Err(CookieError::DomainRequired)
    ));
  }

  #[test]
  fn serialize() {
    let c = ParsedCookie {
      name: "a".to_string(),
      value: "b".to_string(),
      domain: Some("example.com".to_string()),
      path: Some("/sub".to_string()),
      max_age: Some(60),
      expires_ms: Some(784111777000),
      secure: true,
      http_only: true,
      same_site: Some(SameSite::Strict),
      partitioned: true,
    };
    assert_eq!(
      serialize_set_cookie(&c).unwrap(),
      "a=b; Domain=example.com; Path=/sub; Expires=Sun, 06 Nov 1994 08:49:37 GMT; Max-Age=60; Secure; HttpOnly; SameSite=Strict; Partitioned"
    );

    let bad_name = ParsedCookie {
      name: "a b".to_string(),
      value: "b".to_string(),
      ..Default::default()
    };
    assert!(serialize_set_cookie(&bad_name).is_err());
    let bad_value = ParsedCookie {
      name: "a".to_string(),
      value: "b;c".to_string(),
      ..Default::default()
    };
    assert!(serialize_set_cookie(&bad_value).is_err());
    let bad_path = ParsedCookie {
      name: "a".to_string(),
      value: "b".to_string(),
      path: Some("relative".to_string()),
      ..Default::default()
    };
    assert!(serialize_set_cookie(&bad_path).is_err());
  }

  #[test]
  fn cookie_header_parsing() {
    assert_eq!(
      parse_cookie_header("a=1; b=2;c = 3 ; ; d"),
      vec![
        ("a".to_string(), "1".to_string()),
        ("b".to_string(), "2".to_string()),
        ("c".to_string(), "3".to_string()),
      ]
    );
    assert_eq!(parse_cookie_header(""), vec![]);
  }

  #[test]
  fn eviction() {
    let mut store = CookieStore::default();
    let u = url("http://example.com/");
    for i in 0..(MAX_COOKIES_PER_DOMAIN + 10) {
      let c = parse_set_cookie(&format!("c{i}=1")).unwrap();
      store.store_response_cookie(c, &u, NOW + i as i64).unwrap();
    }
    assert_eq!(store.entries(NOW).len(), MAX_COOKIES_PER_DOMAIN);
    // The oldest (least recently used) cookies were evicted.
    assert!(!store.entries(NOW).iter().any(|c| c.name == "c0"));
  }

  #[test]
  fn idna_domains() {
    // Non-ASCII domain attributes are punycoded ("buecher" with u-umlaut).
    let c = parse_set_cookie("a=b; Domain=b\u{fc}cher.example").unwrap();
    let mut store = CookieStore::default();
    store
      .store_response_cookie(c, &url("http://www.xn--bcher-kva.example/"), NOW)
      .unwrap();
    assert_eq!(
      store.cookie_header(&url("http://xn--bcher-kva.example/"), NOW),
      Some("a=b".to_string())
    );
  }
}
