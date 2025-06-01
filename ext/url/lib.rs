// Copyright 2018-2025 the Deno authors. MIT license.

mod urlpattern;

use deno_core::op2;
use deno_core::url::form_urlencoded;
use deno_core::url::quirks;
use deno_core::url::Url;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_error::JsErrorBox;

use crate::urlpattern::op_urlpattern_parse;
use crate::urlpattern::op_urlpattern_process_match_input;

deno_core::extension!(
  deno_url,
  deps = [deno_webidl],
  ops = [
    op_url_reparse,
    op_url_parse,
    op_url_get_serialization,
    op_url_parse_with_base,
    op_url_parse_search_params,
    op_url_stringify_search_params,
    op_urlpattern_parse,
    op_urlpattern_process_match_input
  ],
  esm = ["00_url.js", "01_urlpattern.js"],
);

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
    Err(_) => ParseStatus::Err as u32,
  }
}

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
#[serde]
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
  #[serde] args: Vec<(String, String)>,
) -> String {
  let search = form_urlencoded::Serializer::new(String::new())
    .extend_pairs(args)
    .finish();
  search
}
