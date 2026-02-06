// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::url::form_urlencoded;
use deno_core::url::quirks;
use deno_core::url::Host;
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
  parse_url(state, href, Some(base_href), buf)
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
struct FileUrlQuirks {
  leading_slash_count: usize,
  post_host_slash_count: usize,
  host: Option<Host>
}

impl FileUrlQuirks {
  fn analyze(href: &str, base_host: Option<Host>) -> (Self, &str) {
    let prefix_len = if href.starts_with("file:") { "file:".len() } else { 0 };
 
    let mut leading_slash_count = count_consecutive_slashes(href, prefix_len);
    let rest_after_slashes = &href[(prefix_len + leading_slash_count)..];

    let (href_host, remaining_after_host) = 
      if leading_slash_count == 2 { get_file_host(href) } else { (None, "") };
    
    let mut post_host_slash_count = 0;
    if let Some(ref host) = href_host {
      let after_host_slashes = count_consecutive_slashes(remaining_after_host, 0);

      match host {
        Host::Domain(h) if h == "localhost" => {
          leading_slash_count += after_host_slashes;
        }

        _ => {
          post_host_slash_count = after_host_slashes;
        }
      }
    }

    let host = match leading_slash_count {
      n if n >= 3 => None,

      2 if rest_after_slashes.is_empty() || starts_with_windows_drive_letter(rest_after_slashes) => None,

      2 => href_host.or(base_host),
      
      _ => base_host
    };

    (Self{leading_slash_count, post_host_slash_count, host}, rest_after_slashes)
  }

  fn apply(&self, url: &Url) -> (String, quirks::InternalComponents) {
    let mut serialization: String = url.as_str().to_owned();
    let mut inner_url = quirks::internal_components(url);

    let mut q_start = inner_url.query_start.unwrap_or(0);
    let mut f_start = inner_url.fragment_start.unwrap_or(0);

    // Case A: No host (e.g., file:////)
    if url.host().is_none() && self.leading_slash_count > 2 {
      let extra = (self.leading_slash_count - 3) as u32;
      serialization.insert_str("file:///".len(), &"/".repeat(extra as usize));

      if q_start != 0 { q_start += extra; }
      if f_start != 0 { f_start += extra; }
    }

    // Case B: Host exists (e.g., file://host///)
    if self.post_host_slash_count > 1 {
      let insert_pos = inner_url.host_end as usize;
      let extra = (self.post_host_slash_count - 1) as u32;
      serialization.insert_str(insert_pos, &"/".repeat(extra as usize));

      if q_start != 0 { q_start += extra; }
      if f_start != 0 { f_start += extra; }
    }

    inner_url.query_start = if q_start == 0 { None } else { Some(q_start) };
    inner_url.fragment_start = if f_start == 0 { None } else { Some(f_start) };

    (serialization, inner_url)
  }
}

#[inline]
fn parse_url(
  state: &mut OpState,
  href: &str,
  base_href: Option<&str>,
  buf: &mut [u32],
) -> u32 {
  let mut base_url = None;
  let mut base_file_host = None;

  if let Some(base_url_str) = base_href {
    let url_obj = match Url::parse(base_url_str) {
      Ok(url) => url,
      Err(_) => return ParseStatus::Err as u32,
    };

    if url_obj.scheme() == "file" {
      let (found_host, _) = get_file_host(base_url_str);
      base_file_host = found_host;
    }
    base_url = Some(url_obj);
  }

  let (file_quirks, _) = FileUrlQuirks::analyze(href, base_file_host);

  match Url::options().base_url(base_url.as_ref()).parse(&href) {
    Ok(mut url) => {
      let is_file_url = url.scheme() == "file";
      let is_special = matches!(url.scheme(), "http" | "https" | "ws" | "wss" | "ftp" | "file");

      // Non-special scheme whitespace normalization
      // e.g., "non-special:opaque #hi" -> "non-special:opaque%20#hi"
      if !is_special {
        let s = href.replace(" ?", "%20?").replace(" #", "%20#");
        if let Ok(normalized_url) = Url::options().base_url(base_url.as_ref()).parse(&s) {
          url = normalized_url
        }
      }

      // Restore host for file URLs if missing after standard parsing.
      // Standard parser normalizes "file://host/C:/" to "file:///C:/".
      if is_file_url && url.host().is_none() {
        if let Some(Host::Domain(ref host)) = file_quirks.host {
          if host != "localhost" {
            let _ = quirks::set_host(&mut url, &host);
          }
        }
      }

      // Apply file quirks to restore extra slashes.
      // The standard parser normalizes "file:////" to "file:///".
      let (serialization, inner_url) = if is_file_url {
        file_quirks.apply(&mut url)
      } else {
        (url.as_str().to_owned(), quirks::internal_components(&url))
      };

      let mut host_start = inner_url.host_start;
      let mut host_end = inner_url.host_end;
      let mut port = inner_url.port.unwrap_or(0) as u32;

      if !url.has_authority() {
        host_start = inner_url.path_start;
        host_end = inner_url.path_start;
        port = NO_PORT;
      }

      buf[0] = inner_url.scheme_end;
      buf[1] = inner_url.username_end;
      buf[2] = host_start;
      buf[3] = host_end;
      buf[4] = port;
      buf[5] = inner_url.path_start;
      buf[6] = inner_url.query_start.unwrap_or(0);
      buf[7] = inner_url.fragment_start.unwrap_or(0);

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

#[inline]
fn count_consecutive_slashes(s: &str, offset: usize) -> usize {
  let mut count = 0;
  if let Some(rest) = s.get(offset..) {
    for c in rest.chars() {
      if c == '/' || c == '\\' {
        count += 1;
      } else {
          break;
      }
    }
  }
  count
}

#[inline]
fn get_file_host(url: &str) -> (Option<Host>, &str) {
  let authority = url.strip_prefix("file:").unwrap_or(url);

  let slash_count = count_consecutive_slashes(authority, 0);
  let authority = if slash_count >= 2 {
    &authority[2..]
  } else {
    authority
  };

  let mut bytes = 0;
  let mut has_ignored_chars = false;

  for c in authority.chars() {
    match c {
      '/' | '\\' | '?' | '#' => break,
      '\t' | '\n' | '\r' => has_ignored_chars = true,
      _ => {}
    }
    bytes += c.len_utf8();
  }

  let raw_host = &authority[..bytes];
  let remaining = &authority[bytes..];

  let host_str: Cow<str> = if has_ignored_chars {
    Cow::Owned(raw_host.chars().filter(|&c| !matches!(c, '\t' | '\n' | '\r')).collect())
  } else {
    Cow::Borrowed(raw_host)
  };

  if host_str.is_empty() {
    return (None, remaining);
  }

  if is_windows_drive_letter(&host_str) {
    return (None, url);
  }

  match Host::parse(&host_str) {
      Ok(Host::Domain(ref d)) if d == "localhost" => (Some(Host::Domain(d.clone())), remaining),
      Ok(host) => (Some(host), remaining),
      Err(_) => (None, remaining)
  }
}

#[inline]
fn is_windows_drive_letter(segment: &str) -> bool {
    segment.len() == 2 && starts_with_windows_drive_letter(segment)
}

#[inline]
fn starts_with_windows_drive_letter(s: &str) -> bool {
  let b = s.as_bytes();
    s.len() >= 2
        && (b[0].is_ascii_alphabetic())
        && matches!(b[1], b':' | b'|')
        && (s.len() == 2 || matches!(b[2], b'/' | b'\\' | b'?' | b'#'))
}

#[allow(dead_code)]
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
