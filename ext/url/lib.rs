// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod urlpattern;

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::url::form_urlencoded;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use std::path::PathBuf;

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
#[op]
pub fn op_url_parse_with_base(
  state: &mut OpState,
  href: &str,
  base_href: &str,
  buf: &mut [u32],
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

#[op]
pub fn op_url_get_serialization(state: &mut OpState) -> String {
  state.take::<UrlSerialization>().0
}

/// Parse `href` without a `base_url`. Fills the out `buf` with URL components.
#[op(fast)]
pub fn op_url_parse(state: &mut OpState, href: &str, buf: &mut [u32]) -> u32 {
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
  base_href: Option<&str>,
  buf: &mut [u32],
) -> u32 {
  match deno_ada_rs::parse(href, base_href) {
    Ok((inner_url, href)) => {
      buf[0] = inner_url.protocol_end;
      buf[1] = inner_url.username_end;
      buf[2] = inner_url.host_start;
      buf[3] = inner_url.host_end;
      buf[4] = inner_url.port;
      buf[5] = inner_url.pathname_start;
      buf[6] = inner_url.search_start;
      buf[7] = inner_url.hash_start;

      if inner_url.same_as_input == 0 {
        state.put(UrlSerialization(href.to_string()));
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

fn as_u32_slice(slice: &mut [u8]) -> &mut [u32] {
  assert_eq!(slice.len() % std::mem::size_of::<u32>(), 0);
  // SAFETY: size is multiple of 4
  unsafe {
    std::slice::from_raw_parts_mut(
      slice.as_mut_ptr() as *mut u32,
      slice.len() / std::mem::size_of::<u32>(),
    )
  }
}

#[op]
pub fn op_url_reparse(
  state: &mut OpState,
  href: &str,
  setter: u8,
  value: &str,
  buf: &mut [u8],
) -> u32 {
  if setter > 8 {
    return ParseStatus::Err as u32;
  }
  // SAFETY: checked to be less than 9.
  let setter = unsafe { std::mem::transmute::<u8, UrlSetter>(setter) };
  let e = match setter {
    UrlSetter::Hash => deno_ada_rs::set_hash(href, value),
    UrlSetter::Host => deno_ada_rs::set_host(href, value),
    UrlSetter::Hostname => deno_ada_rs::set_hostname(href, value),

    UrlSetter::Password => deno_ada_rs::set_password(href, value),

    UrlSetter::Pathname => deno_ada_rs::set_pathname(href, value),
    UrlSetter::Port => deno_ada_rs::set_port(href, value),

    UrlSetter::Protocol => deno_ada_rs::set_protocol(href, value),
    UrlSetter::Search => deno_ada_rs::set_search(href, value),
    UrlSetter::Username => deno_ada_rs::set_username(href, value),
  };

  match e {
    Ok((inner_url, href)) => {
      let buf: &mut [u32] = as_u32_slice(buf);
      buf[0] = inner_url.protocol_end;
      buf[1] = inner_url.username_end;
      buf[2] = inner_url.host_start;
      buf[3] = inner_url.host_end;
      buf[4] = inner_url.port;
      buf[5] = inner_url.pathname_start;
      buf[6] = inner_url.search_start;
      buf[7] = inner_url.hash_start;

      if inner_url.same_as_input == 0 {
        state.put(UrlSerialization(href));
        ParseStatus::OkSerialization as u32
      } else {
        ParseStatus::Ok as u32
      }
    }
    Err(_) => ParseStatus::Err as u32,
  }
}

#[op]
pub fn op_url_parse_search_params(
  args: Option<String>,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Vec<(String, String)>, AnyError> {
  let params = match (args, zero_copy) {
    (None, Some(zero_copy)) => form_urlencoded::parse(&zero_copy)
      .into_iter()
      .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned()))
      .collect(),
    (Some(args), None) => form_urlencoded::parse(args.as_bytes())
      .into_iter()
      .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned()))
      .collect(),
    _ => return Err(type_error("invalid parameters")),
  };
  Ok(params)
}

#[op]
pub fn op_url_stringify_search_params(args: Vec<(String, String)>) -> String {
  let search = form_urlencoded::Serializer::new(String::new())
    .extend_pairs(args)
    .finish();
  search
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_url.d.ts")
}
