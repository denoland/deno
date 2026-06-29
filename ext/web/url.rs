// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_core::url::Host;
use deno_core::url::ParseError;
use deno_core::url::Url;
use deno_core::url::form_urlencoded;
use deno_core::url::quirks;
use deno_error::JsErrorBox;
use percent_encoding::percent_decode;

/// Parse `href` with a `base_href`. Fills the out `buf` with URL components.
#[op2(fast)]
#[smi]
pub fn op_url_parse_with_base(
  state: &mut OpState,
  #[string] href: &str,
  #[string] base_href: &str,
  #[buffer] buf: &mut [u32],
) -> u32 {
  let base_url = match Url::parse(base_href) {
    Ok(url) => url,
    Err(_) => return ParseStatus::Err as u32,
  };
  parse_url(state, href, Some(&base_url), buf)
}

#[repr(u32)]
pub enum ParseStatus {
  Ok = 0,
  OkSerialization = 1,
  Err,
}

struct UrlSerialization(String);

#[op2]
#[string]
pub fn op_url_get_serialization(state: &mut OpState) -> String {
  state.take::<UrlSerialization>().0
}

/// Parse `href` without a `base_url`. Fills the out `buf` with URL components.
#[op2(fast)]
#[smi]
pub fn op_url_parse(
  state: &mut OpState,
  #[string] href: &str,
  #[buffer] buf: &mut [u32],
) -> u32 {
  if parse_simple_special_url(href, buf) {
    return ParseStatus::Ok as u32;
  }
  parse_url(state, href, None, buf)
}

/// `op_url_parse` and `op_url_parse_with_base` share the same implementation.
///
/// This function is used to parse the URL and fill the `buf` with internal
/// offset values of the URL components.
///
/// If the serialized URL is the same as the input URL, then `UrlSerialization` is
/// not set and returns `ParseStatus::Ok`.
///
/// If the serialized URL is different from the input URL, then `UrlSerialization` is
/// set and returns `ParseStatus::OkSerialization`. JS side should check status and
/// use `op_url_get_serialization` to get the serialized URL.
///
/// If the URL is invalid, then `UrlSerialization` is not set and returns `ParseStatus::Err`.
///
/// ```js
/// const buf = new Uint32Array(8);
/// const status = op_url_parse("http://example.com", buf.buffer);
/// let serializedUrl = "";
/// if (status === ParseStatus.Ok) {
///   serializedUrl = "http://example.com";
/// } else if (status === ParseStatus.OkSerialization) {
///   serializedUrl = op_url_get_serialization();
/// }
/// ```
#[inline]
fn parse_url(
  state: &mut OpState,
  href: &str,
  base_href: Option<&Url>,
  buf: &mut [u32],
) -> u32 {
  match Url::options().base_url(base_href).parse(href) {
    Ok(url) => {
      let inner_url = quirks::internal_components(&url);

      buf[0] = inner_url.scheme_end;
      buf[1] = inner_url.username_end;
      buf[2] = inner_url.host_start;
      buf[3] = inner_url.host_end;
      buf[4] = inner_url.port.unwrap_or(0) as u32;
      buf[5] = inner_url.path_start;
      buf[6] = inner_url.query_start.unwrap_or(0);
      buf[7] = inner_url.fragment_start.unwrap_or(0);
      let serialization: String = url.into();
      if serialization != href {
        state.put(UrlSerialization(serialization));
        ParseStatus::OkSerialization as u32
      } else {
        ParseStatus::Ok as u32
      }
    }
    // Per whatwg/url#914: a non-strict, all-ASCII domain whose Unicode ToASCII
    // step fails (e.g. a bogus `xn--` label) must be accepted as the lowercased
    // percent-decoded host as-is, rather than throwing. rust-url's `Host::parse`
    // surfaces this as `IdnaError`; fall back to building the serialization
    // ourselves via the sentinel-substitution mechanism.
    Err(ParseError::IdnaError) => {
      ascii_idna_fallback_parse(state, href, base_href, buf)
    }
    Err(_) => ParseStatus::Err as u32,
  }
}

/// Strip ASCII tab/LF/CR (the code points the WHATWG URL parser removes from the
/// input before host parsing).
fn is_tab_or_newline(b: u8) -> bool {
  matches!(b, b'\t' | b'\n' | b'\r')
}

/// A forbidden domain code point per
/// <https://url.spec.whatwg.org/#forbidden-domain-code-point>: a forbidden host
/// code point (C0 controls + space, U+007F, and `# / : < > ? @ [ \ ] ^ |`), plus
/// `%`. A host containing any of these is fatal even in the ASCII fallback.
fn is_forbidden_domain_code_point(b: u8) -> bool {
  b <= 0x20
    || b == 0x7f
    || matches!(
      b,
      b'#'
        | b'%'
        | b'/'
        | b':'
        | b'<'
        | b'>'
        | b'?'
        | b'@'
        | b'['
        | b'\\'
        | b']'
        | b'^'
        | b'|'
    )
}

/// Compute the ASCII "domain to ASCII" fallback host for `raw_host`, the raw host
/// substring taken from the input. Returns `Some(host)` ONLY when the host is a
/// non-empty, all-ASCII domain with no forbidden domain code point whose
/// `Host::parse` fails specifically with `IdnaError` (the bogus-`xn--` case). The
/// `Host::parse == Err(IdnaError)` gate is folded in here so EVERY call site is
/// gated identically — e.g. an invalid IPv4 like `1.2.3.4.5` parses to
/// `InvalidIpv4Address`, not `IdnaError`, so it returns `None` everywhere and
/// still throws / no-ops.
fn ascii_domain_fallback_host(raw_host: &str) -> Option<String> {
  let stripped: Vec<u8> = raw_host
    .bytes()
    .filter(|&b| !is_tab_or_newline(b))
    .collect();
  let decoded = percent_decode(&stripped).collect::<Vec<u8>>();
  if decoded.is_empty() || !decoded.is_ascii() {
    return None;
  }
  if decoded.iter().any(|&b| is_forbidden_domain_code_point(b)) {
    return None;
  }
  // All bytes are ASCII, so this is valid UTF-8.
  let candidate = String::from_utf8(decoded).ok()?.to_ascii_lowercase();
  match Host::parse(&candidate) {
    Err(ParseError::IdnaError) => Some(candidate),
    _ => None,
  }
}

/// Find the start of the authority within `bytes`, for both the absolute form
/// (`scheme://authority`) and the scheme-relative authority form (`//authority`,
/// only valid when a base URL is present). Returns the byte offset just after the
/// leading two slashes, or `None` when there is no authority (e.g. a relative or
/// path-only input).
fn scheme_authority_start(bytes: &[u8], base_present: bool) -> Option<usize> {
  // Trim leading C0 controls and space (the WHATWG parser strips these).
  let mut start = 0;
  while start < bytes.len() && bytes[start] <= 0x20 {
    start += 1;
  }
  // Absolute form: ALPHA *(ALPHA / DIGIT / "+" / "-" / ".") ":" then "//".
  if start < bytes.len() && bytes[start].is_ascii_alphabetic() {
    let mut i = start + 1;
    while i < bytes.len() {
      match bytes[i] {
        b':' => return skip_two_slashes(bytes, i + 1),
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'+' | b'-' | b'.' => {
          i += 1;
        }
        _ => break,
      }
    }
  }
  // Scheme-relative authority form (`//authority`), only when a base is present.
  if base_present {
    return skip_two_slashes(bytes, start);
  }
  None
}

/// Skip exactly two authority slashes (`/` or `\`), transparent to interleaved
/// ASCII tab/LF/CR, returning the offset just after the second slash. Anything
/// else means there is no authority (path-relative input).
fn skip_two_slashes(bytes: &[u8], start: usize) -> Option<usize> {
  let mut i = start;
  let mut slashes = 0;
  while i < bytes.len() {
    match bytes[i] {
      b'/' | b'\\' => {
        slashes += 1;
        i += 1;
        if slashes == 2 {
          return Some(i);
        }
      }
      b'\t' | b'\n' | b'\r' => i += 1,
      _ => return None,
    }
  }
  None
}

/// Locate the host byte span within `href`. Handles both the absolute and (when
/// `base_present`) scheme-relative authority forms. The authority runs to the
/// first `/ \ ? #`; the host starts after the LAST `@` and ends at the first `:`.
/// Returns `None` for an IPv6 host (`[`), an empty host, or a non-authority
/// input.
fn find_host_span(href: &str, base_present: bool) -> Option<(usize, usize)> {
  let bytes = href.as_bytes();
  let authority_start = scheme_authority_start(bytes, base_present)?;
  let mut authority_end = authority_start;
  while authority_end < bytes.len() {
    match bytes[authority_end] {
      b'/' | b'\\' | b'?' | b'#' => break,
      _ => authority_end += 1,
    }
  }
  let authority = &bytes[authority_start..authority_end];
  // The host starts after the LAST `@` (userinfo delimiter).
  let host_start = match authority.iter().rposition(|&b| b == b'@') {
    Some(at) => authority_start + at + 1,
    None => authority_start,
  };
  // The host ends at the first `:` (port delimiter).
  let mut host_end = authority_end;
  let mut i = host_start;
  while i < authority_end {
    if bytes[i] == b':' {
      host_end = i;
      break;
    }
    i += 1;
  }
  if host_start >= host_end || bytes[host_start] == b'[' {
    return None;
  }
  Some((host_start, host_end))
}

/// Rebuild the serialization of a sentinel-host `Url` (host == "a") by splicing
/// the real `fallback` host into the host span, then shift every component offset
/// after the host by `fallback.len() - 1`. Fills `buf` and stores the rebuilt
/// serialization. `port_none` is the sentinel written to `buf[4]` when the URL
/// has no port (0 on the parse path, `NO_PORT` on the reparse path).
fn swap_sentinel_host(
  state: &mut OpState,
  url: &Url,
  fallback: &str,
  port_none: u32,
  href: &str,
  buf: &mut [u32],
) -> u32 {
  let inner = quirks::internal_components(url);
  let s = url.as_str();
  let hs = inner.host_start as usize;
  let he = inner.host_end as usize;
  // Sanity check: the sentinel host must be the single char "a".
  if s.get(hs..he) != Some("a") {
    return ParseStatus::Err as u32;
  }
  let mut out = String::with_capacity(s.len() + fallback.len());
  out.push_str(&s[..hs]);
  out.push_str(fallback);
  out.push_str(&s[he..]);

  let shift = fallback.len() as u32 - 1;
  buf[0] = inner.scheme_end;
  buf[1] = inner.username_end;
  buf[2] = inner.host_start;
  buf[3] = inner.host_end + shift;
  buf[4] = inner.port.map(|p| p as u32).unwrap_or(port_none);
  buf[5] = inner.path_start + shift;
  buf[6] = inner.query_start.map(|q| q + shift).unwrap_or(0);
  buf[7] = inner.fragment_start.map(|f| f + shift).unwrap_or(0);

  if out != href {
    state.put(UrlSerialization(out));
    ParseStatus::OkSerialization as u32
  } else {
    ParseStatus::Ok as u32
  }
}

/// The ASCII fallback for `op_url_parse` / `op_url_parse_with_base`: find the
/// host span, compute the fallback host, parse a sentinel-host copy against the
/// same base so rust-url canonicalizes everything else, then splice the real host
/// back in.
fn ascii_idna_fallback_parse(
  state: &mut OpState,
  href: &str,
  base_href: Option<&Url>,
  buf: &mut [u32],
) -> u32 {
  let Some((host_start, host_end)) = find_host_span(href, base_href.is_some())
  else {
    return ParseStatus::Err as u32;
  };
  let Some(fallback) = ascii_domain_fallback_host(&href[host_start..host_end])
  else {
    return ParseStatus::Err as u32;
  };
  let mut sentinel = String::with_capacity(href.len());
  sentinel.push_str(&href[..host_start]);
  sentinel.push('a');
  sentinel.push_str(&href[host_end..]);
  match Url::options().base_url(base_href).parse(&sentinel) {
    Ok(url) => swap_sentinel_host(state, &url, &fallback, 0, href, buf),
    Err(_) => ParseStatus::Err as u32,
  }
}

// Keep in sync with parseSimpleSpecialUrl() in ext/web/00_url.js.
fn parse_simple_special_url(href: &str, buf: &mut [u32]) -> bool {
  let bytes = href.as_bytes();
  let (scheme_end, default_port) = if bytes.starts_with(b"http://") {
    (4, 80u32)
  } else if bytes.starts_with(b"https://") {
    (5, 443u32)
  } else {
    return false;
  };

  let host_start = scheme_end + 3;
  let mut path_start = host_start;
  while path_start < bytes.len() && bytes[path_start] != b'/' {
    match bytes[path_start] {
      b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b':' => {
        path_start += 1;
      }
      _ => return false,
    }
  }
  if path_start == host_start || path_start == bytes.len() {
    return false;
  }

  let mut host_end = path_start;
  let mut port = 65536u32;
  for i in host_start..path_start {
    if bytes[i] == b':' {
      if i == host_start || i + 1 == path_start {
        return false;
      }
      host_end = i;
      port = 0;
      if i + 2 < path_start && bytes[i + 1] == b'0' {
        return false;
      }
      for &byte in &bytes[i + 1..path_start] {
        if !byte.is_ascii_digit() {
          return false;
        }
        let Some(next_port) = port
          .checked_mul(10)
          .and_then(|port| port.checked_add((byte - b'0') as u32))
        else {
          return false;
        };
        if next_port > 65535 {
          return false;
        }
        port = next_port;
      }
      if port == default_port {
        return false;
      }
      break;
    }
  }
  if !simple_special_host_is_canonical(&bytes[host_start..host_end]) {
    return false;
  }

  let mut query_start = 0u32;
  for i in path_start..bytes.len() {
    if bytes[i] == b'.'
      && i > path_start
      && bytes[i - 1] == b'/'
      && (i + 1 == bytes.len() || matches!(bytes[i + 1], b'/' | b'?' | b'.'))
    {
      return false;
    }
    if query_start != 0 && bytes[i] == b'\'' {
      return false;
    }
    match bytes[i] {
      b'a'..=b'z'
      | b'A'..=b'Z'
      | b'0'..=b'9'
      | b'/'
      | b'.'
      | b'_'
      | b'~'
      | b'-'
      | b'!'
      | b'$'
      | b'&'
      | b'\''
      | b'('
      | b')'
      | b'*'
      | b'+'
      | b','
      | b';'
      | b'='
      | b':'
      | b'@' => {}
      b'?' if query_start == 0 => query_start = i as u32,
      _ => return false,
    }
  }

  buf[0] = scheme_end as u32;
  buf[1] = host_start as u32;
  buf[2] = host_start as u32;
  buf[3] = host_end as u32;
  buf[4] = port;
  buf[5] = path_start as u32;
  buf[6] = query_start;
  buf[7] = 0;
  true
}

fn simple_special_host_is_canonical(host: &[u8]) -> bool {
  if host.is_empty() || host[0] == b'.' || host[host.len() - 1] == b'.' {
    return false;
  }

  if host
    .iter()
    .all(|byte| byte.is_ascii_digit() || *byte == b'.')
  {
    let mut dots = 0;
    let mut part = 0u32;
    let mut part_len = 0;
    for (i, &byte) in host.iter().enumerate() {
      if byte == b'.' {
        if part_len == 0
          || part > 255
          || (part_len > 1 && host[i - part_len] == b'0')
        {
          return false;
        }
        dots += 1;
        part = 0;
        part_len = 0;
        continue;
      }
      part = part * 10 + (byte - b'0') as u32;
      part_len += 1;
      if part_len > 3 {
        return false;
      }
    }
    return dots == 3
      && part_len != 0
      && part <= 255
      && (part_len == 1 || host[host.len() - part_len] != b'0');
  }

  if host[0].is_ascii_digit() {
    return false;
  }

  let mut label_len = 0;
  let mut label_start = 0;
  let mut final_label_all_digits = true;
  for (i, &byte) in host.iter().enumerate() {
    match byte {
      b'a'..=b'z' | b'0'..=b'9' | b'-' => {
        if !byte.is_ascii_digit() {
          final_label_all_digits = false;
        }
        label_len += 1;
      }
      b'.' => {
        if label_len == 0 {
          return false;
        }
        label_len = 0;
        label_start = i + 1;
        final_label_all_digits = true;
      }
      _ => return false,
    }
    if i == label_start + 3 && &host[label_start..=i] == b"xn--" {
      return false;
    }
  }
  label_len != 0
    && !final_label_all_digits
    && !(label_len >= 2 && &host[label_start..label_start + 2] == b"0x")
}

#[allow(dead_code, reason = "used in JS")]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum UrlSetter {
  Hash = 0,
  Host = 1,
  Hostname = 2,
  Password = 3,
  Pathname = 4,
  Port = 5,
  Protocol = 6,
  Search = 7,
  Username = 8,
}

const NO_PORT: u32 = 65536;

#[op2(fast)]
#[smi]
pub fn op_url_reparse(
  state: &mut OpState,
  #[string] href: String,
  #[smi] setter: u8,
  #[string] setter_value: String,
  #[buffer] buf: &mut [u32],
) -> u32 {
  if setter > 8 {
    return ParseStatus::Err as u32;
  }
  // SAFETY: checked to be less than 9.
  let setter = unsafe { std::mem::transmute::<u8, UrlSetter>(setter) };

  let mut url = match Url::options().parse(&href) {
    Ok(url) => url,
    // The current URL is itself a bogus-`xn--` fallback-host serialization that
    // rust-url re-rejects (e.g. `https://xn--bogus.../...`). Apply the setter on
    // a sentinel-host copy and re-patch the fallback host so the URL stays
    // mutable. See `ascii_idna_fallback_reparse`.
    Err(ParseError::IdnaError) => {
      return ascii_idna_fallback_reparse(
        state,
        &href,
        setter,
        &setter_value,
        buf,
      );
    }
    Err(_) => return ParseStatus::Err as u32,
  };

  let value = setter_value.as_str();
  let e = match setter {
    UrlSetter::Hash => {
      quirks::set_hash(&mut url, value);
      Ok(())
    }
    UrlSetter::Host => quirks::set_host(&mut url, value),

    UrlSetter::Hostname => quirks::set_hostname(&mut url, value),

    UrlSetter::Password => quirks::set_password(&mut url, value),

    UrlSetter::Pathname => {
      quirks::set_pathname(&mut url, value);
      Ok(())
    }
    UrlSetter::Port => quirks::set_port(&mut url, value),

    UrlSetter::Protocol => quirks::set_protocol(&mut url, value),
    UrlSetter::Search => {
      quirks::set_search(&mut url, value);
      Ok(())
    }
    UrlSetter::Username => quirks::set_username(&mut url, value),
  };

  match e {
    Ok(_) => {
      let inner_url = quirks::internal_components(&url);

      buf[0] = inner_url.scheme_end;
      buf[1] = inner_url.username_end;
      buf[2] = inner_url.host_start;
      buf[3] = inner_url.host_end;
      buf[4] = inner_url.port.map(|p| p as u32).unwrap_or(NO_PORT);
      buf[5] = inner_url.path_start;
      buf[6] = inner_url.query_start.unwrap_or(0);
      buf[7] = inner_url.fragment_start.unwrap_or(0);
      let serialization: String = url.into();
      if serialization != href {
        state.put(UrlSerialization(serialization));
        ParseStatus::OkSerialization as u32
      } else {
        ParseStatus::Ok as u32
      }
    }
    // A Host/Hostname setter that failed may be setting a bogus-`xn--` ASCII
    // host on an otherwise-normal URL; try the ASCII fallback before giving up.
    // Every other failed setter is a genuine error.
    Err(_) => match setter {
      UrlSetter::Host | UrlSetter::Hostname => reparse_host_setter_fallback(
        state,
        &mut url,
        setter,
        &setter_value,
        buf,
      ),
      _ => ParseStatus::Err as u32,
    },
  }
}

/// Split a host/hostname setter value into its host substring, (host setter
/// only) port substring, and whether the host was terminated by a `:`,
/// replicating WHATWG host/hostname-state termination: strip ASCII tab/LF/CR,
/// then take the host up to the first of `: / \ ? #`. For the host setter, if the
/// terminator is `:`, the port runs from after it to the first of `/ \ ? #`. The
/// hostname setter never yields a port, and a `:` terminator means the hostname
/// state aborts (a no-op), so callers use the returned flag to skip the fallback.
fn split_setter_value(
  setter: UrlSetter,
  value: &str,
) -> (String, Option<String>, bool) {
  let stripped: String = value
    .chars()
    .filter(|&c| !matches!(c, '\t' | '\n' | '\r'))
    .collect();
  let bytes = stripped.as_bytes();
  let mut i = 0;
  let mut colon = None;
  while i < bytes.len() {
    match bytes[i] {
      b':' => {
        colon = Some(i);
        break;
      }
      b'/' | b'\\' | b'?' | b'#' => break,
      _ => i += 1,
    }
  }
  let host_str = stripped[..i].to_string();
  let colon_terminated = colon.is_some();
  if setter == UrlSetter::Host
    && let Some(c) = colon
  {
    let mut j = c + 1;
    while j < bytes.len() {
      match bytes[j] {
        b'/' | b'\\' | b'?' | b'#' => break,
        _ => j += 1,
      }
    }
    (
      host_str,
      Some(stripped[c + 1..j].to_string()),
      colon_terminated,
    )
  } else {
    (host_str, None, colon_terminated)
  }
}

/// Apply a non-host setter to a sentinel-host `Url`. Host/Hostname are handled
/// separately and never reach here.
fn apply_simple_setter(
  setter: UrlSetter,
  url: &mut Url,
  value: &str,
) -> Result<(), ()> {
  match setter {
    UrlSetter::Hash => {
      quirks::set_hash(url, value);
      Ok(())
    }
    UrlSetter::Pathname => {
      quirks::set_pathname(url, value);
      Ok(())
    }
    UrlSetter::Search => {
      quirks::set_search(url, value);
      Ok(())
    }
    UrlSetter::Port => quirks::set_port(url, value),
    UrlSetter::Username => quirks::set_username(url, value),
    UrlSetter::Password => quirks::set_password(url, value),
    UrlSetter::Protocol => quirks::set_protocol(url, value),
    UrlSetter::Host | UrlSetter::Hostname => Err(()),
  }
}

/// Fill `buf` from `url` directly (host already valid, no sentinel swap) and
/// store the serialization, using the reparse `NO_PORT` convention.
fn finish_plain(
  state: &mut OpState,
  url: &Url,
  href: &str,
  buf: &mut [u32],
) -> u32 {
  let inner = quirks::internal_components(url);
  buf[0] = inner.scheme_end;
  buf[1] = inner.username_end;
  buf[2] = inner.host_start;
  buf[3] = inner.host_end;
  buf[4] = inner.port.map(|p| p as u32).unwrap_or(NO_PORT);
  buf[5] = inner.path_start;
  buf[6] = inner.query_start.unwrap_or(0);
  buf[7] = inner.fragment_start.unwrap_or(0);
  let serialization: String = url.as_str().to_string();
  if serialization != href {
    state.put(UrlSerialization(serialization));
    ParseStatus::OkSerialization as u32
  } else {
    ParseStatus::Ok as u32
  }
}

/// Reparse a setter when the CURRENT URL is itself a bogus-`xn--` fallback-host
/// serialization (the initial `Url::parse` returned `IdnaError`). Rebuild a
/// sentinel-host `Url` from the current href, apply the setter, then re-patch the
/// host: a host/hostname change that becomes a valid host serializes directly; a
/// host/hostname change that stays bogus, or any other setter, keeps the host as
/// the sentinel and swaps the real fallback host back in.
fn ascii_idna_fallback_reparse(
  state: &mut OpState,
  href: &str,
  setter: UrlSetter,
  setter_value: &str,
  buf: &mut [u32],
) -> u32 {
  let Some((host_start, host_end)) = find_host_span(href, false) else {
    return ParseStatus::Err as u32;
  };
  let orig_host = href[host_start..host_end].to_string();
  let mut sentinel = String::with_capacity(href.len());
  sentinel.push_str(&href[..host_start]);
  sentinel.push('a');
  sentinel.push_str(&href[host_end..]);
  let mut url = match Url::options().parse(&sentinel) {
    Ok(url) => url,
    Err(_) => return ParseStatus::Err as u32,
  };

  match setter {
    UrlSetter::Host | UrlSetter::Hostname => {
      let (host_str, port_str, colon_terminated) =
        split_setter_value(setter, setter_value);
      // The WHATWG hostname state aborts (no-op) on a `:`; match the behavior of
      // a valid host (`u.hostname = "example.org:8080"` is also a no-op).
      if setter == UrlSetter::Hostname && colon_terminated {
        return ParseStatus::Err as u32;
      }
      // First try a normal host change with the terminated host substring: if it
      // is a valid host, the URL is no longer a fallback host and serializes
      // directly.
      if quirks::set_hostname(&mut url, &host_str).is_ok() {
        if setter == UrlSetter::Host
          && let Some(port) = &port_str
          && !port.is_empty()
          && quirks::set_port(&mut url, port).is_err()
        {
          return ParseStatus::Err as u32;
        }
        return finish_plain(state, &url, href, buf);
      }
      // The new host is itself a bogus-`xn--` value: swap in the NEW fallback.
      let Some(fallback) = ascii_domain_fallback_host(&host_str) else {
        return ParseStatus::Err as u32;
      };
      if quirks::set_hostname(&mut url, "a").is_err() {
        return ParseStatus::Err as u32;
      }
      if setter == UrlSetter::Host
        && let Some(port) = &port_str
        && !port.is_empty()
        && quirks::set_port(&mut url, port).is_err()
      {
        return ParseStatus::Err as u32;
      }
      swap_sentinel_host(state, &url, &fallback, NO_PORT, href, buf)
    }
    other => match apply_simple_setter(other, &mut url, setter_value) {
      Ok(()) => swap_sentinel_host(state, &url, &orig_host, NO_PORT, href, buf),
      Err(()) => ParseStatus::Err as u32,
    },
  }
}

/// Reparse a Host/Hostname setter that failed on an otherwise-normal URL because
/// the value is a bogus-`xn--` ASCII host. Set the host to the sentinel, apply
/// the port (host setter only), then swap in the value's fallback host.
fn reparse_host_setter_fallback(
  state: &mut OpState,
  url: &mut Url,
  setter: UrlSetter,
  setter_value: &str,
  buf: &mut [u32],
) -> u32 {
  let (host_str, port_str, colon_terminated) =
    split_setter_value(setter, setter_value);
  // The WHATWG hostname state aborts (no-op) on a `:`, like a valid host.
  if setter == UrlSetter::Hostname && colon_terminated {
    return ParseStatus::Err as u32;
  }
  let Some(fallback) = ascii_domain_fallback_host(&host_str) else {
    return ParseStatus::Err as u32;
  };
  if quirks::set_hostname(url, "a").is_err() {
    return ParseStatus::Err as u32;
  }
  if setter == UrlSetter::Host
    && let Some(port) = &port_str
    && !port.is_empty()
    && quirks::set_port(url, port).is_err()
  {
    return ParseStatus::Err as u32;
  }
  // The serialization always differs from the original href (the host changed),
  // so the equality check never matters; pass an empty string.
  swap_sentinel_host(state, url, &fallback, NO_PORT, "", buf)
}

#[op2]
pub fn op_url_parse_search_params(
  #[string] args: Option<String>,
  #[buffer] zero_copy: Option<JsBuffer>,
) -> Result<Vec<(String, String)>, JsErrorBox> {
  let params = match (args, zero_copy) {
    (None, Some(zero_copy)) => form_urlencoded::parse(&zero_copy)
      .into_iter()
      .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned()))
      .collect(),
    (Some(args), None) => form_urlencoded::parse(args.as_bytes())
      .into_iter()
      .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned()))
      .collect(),
    _ => return Err(JsErrorBox::type_error("invalid parameters")),
  };
  Ok(params)
}

#[op2]
#[string]
pub fn op_url_stringify_search_params(
  #[scoped] args: Vec<(String, String)>,
) -> String {
  form_urlencoded::Serializer::new(String::new())
    .extend_pairs(args)
    .finish()
}
