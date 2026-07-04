// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::url::form_urlencoded;
use deno_core::url::quirks;
use deno_error::JsErrorBox;

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
  let href_to_parse = match base_href {
    Some(base_url) => normalize_special_scheme_relative_url(href, base_url),
    None => Cow::Borrowed(href),
  };

  match Url::options().base_url(base_href).parse(&href_to_parse) {
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
    Err(_) => ParseStatus::Err as u32,
  }
}

fn normalize_special_scheme_relative_url<'a>(
  href: &'a str,
  base_url: &Url,
) -> Cow<'a, str> {
  // WHATWG resolves extra-leading-slash network-path references like
  // `///host` to the base's special scheme, but rust-url rejects them.
  if !href.as_bytes().starts_with(b"///")
    || !matches!(base_url.scheme(), "ftp" | "http" | "https" | "ws" | "wss")
  {
    return Cow::Borrowed(href);
  }

  Cow::Owned(format!(
    "{}://{}",
    base_url.scheme(),
    href.trim_start_matches('/')
  ))
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
#[derive(Eq, PartialEq, Debug)]
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
  let mut url = match Url::options().parse(&href) {
    Ok(url) => url,
    Err(_) => return ParseStatus::Err as u32,
  };

  if setter > 8 {
    return ParseStatus::Err as u32;
  }
  // SAFETY: checked to be less than 9.
  let setter = unsafe { std::mem::transmute::<u8, UrlSetter>(setter) };
  let value = setter_value.as_ref();
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
    Err(_) => ParseStatus::Err as u32,
  }
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
