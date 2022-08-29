// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod urlpattern;

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::url::form_urlencoded;
use deno_core::url::quirks;
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::ZeroCopyBuf;
use std::mem::transmute;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use crate::urlpattern::op_urlpattern_parse;
use crate::urlpattern::op_urlpattern_process_match_input;

pub fn init() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/url",
      "00_url.js",
      "01_urlpattern.js",
    ))
    .ops(vec![
      op_url_parse::decl(),
      op_url_reparse::decl(),
      op_url_parse::decl(),
      op_url_set_buf::decl(),
      op_url_get_serialization::decl(),
      op_url_parse_with_base::decl(),
      op_url_parse_search_params::decl(),
      op_url_stringify_search_params::decl(),
      op_urlpattern_parse::decl(),
      op_urlpattern_process_match_input::decl(),
    ])
    .build()
}

static mut URL_OFFSET_BUF: *mut u32 = std::ptr::null_mut();

#[op]
pub fn op_url_set_buf(buf: ZeroCopyBuf) {
  assert_eq!(buf.len(), 32);
  // SAFETY: This is safe because this is the only place where we initialize
  // NOW_BUF.
  unsafe {
    URL_OFFSET_BUF = buf.as_ptr() as *mut u32;
  }
}

#[allow(dead_code)]
enum HostInternal {
  None,
  Domain,
  Ipv4(Ipv4Addr),
  Ipv6(Ipv6Addr),
}

pub struct InnerUrl {
  /// Syntax in pseudo-BNF:
  ///
  ///   url = scheme ":" [ hierarchical | non-hierarchical ] [ "?" query ]? [ "#" fragment ]?
  ///   non-hierarchical = non-hierarchical-path
  ///   non-hierarchical-path = /* Does not start with "/" */
  ///   hierarchical = authority? hierarchical-path
  ///   authority = "//" userinfo? host [ ":" port ]?
  ///   userinfo = username [ ":" password ]? "@"
  ///   hierarchical-path = [ "/" path-segment ]+
  serialization: String,
  // Components
  scheme_end: u32,   // Before ':'
  username_end: u32, // Before ':' (if a password is given) or '@' (if not)
  host_start: u32,
  host_end: u32,
  _host: HostInternal,
  port: Option<u16>,
  path_start: u32,             // Before initial '/', if any
  query_start: Option<u32>,    // Before '?', unlike Position::QueryStart
  fragment_start: Option<u32>, // Before '#', unlike Position::FragmentStart
}

const _: () = {
  assert!(std::mem::size_of::<InnerUrl>() == std::mem::size_of::<Url>());
};

/// Parse `UrlParseArgs::href` with an optional `UrlParseArgs::base_href`, or an
/// optional part to "set" after parsing. Return `UrlParts`.
#[op]
pub fn op_url_parse_with_base(href: String, base_href: String) -> u32 {
  let base_url = match Url::parse(&base_href) {
    Ok(url) => url,
    Err(_) => return ParseStatus::Err as u32,
  };
  parse_url(href, Some(&base_url))
}

#[repr(u32)]
pub enum ParseStatus {
  Ok = 0,
  OkSerialization = 1,
  Err,
}

static mut URL_SERIALIZATION: Option<String> = None;

#[op]
pub fn op_url_get_serialization() -> String {
  unsafe { URL_SERIALIZATION.take().unwrap_or_default() }
}

#[op]
pub fn op_url_parse(href: String) -> u32 {
  parse_url(href, None)
}

#[inline]
fn parse_url(href: String, base_href: Option<&Url>) -> u32 {
  match Url::options().base_url(base_href).parse(&href) {
    Ok(url) => {
      let inner_url: InnerUrl = unsafe { transmute(url) };

      // SAFETY: This is safe because we initialize URL_OFFSET_BUF in op_url_set_buf, its a null pointer
      // otherwise.
      // op_url_set_buf guarantees that the buffer is 4 * 8 bytes long.
      unsafe {
        if !URL_OFFSET_BUF.is_null() {
          let buf = std::slice::from_raw_parts_mut(URL_OFFSET_BUF, 8);
          buf[0] = inner_url.scheme_end;
          buf[1] = inner_url.username_end;
          buf[2] = inner_url.host_start;
          buf[3] = inner_url.host_end;
          buf[4] = inner_url.port.unwrap_or(0) as u32;
          buf[5] = inner_url.path_start;
          buf[6] = inner_url.query_start.unwrap_or(0);
          buf[7] = inner_url.fragment_start.unwrap_or(0);
        }
        if inner_url.serialization != href {
          URL_SERIALIZATION.replace(inner_url.serialization);
          ParseStatus::OkSerialization as u32
        } else {
          ParseStatus::Ok as u32
        }
      }
    }
    Err(_) => ParseStatus::Err as u32,
  }
}

#[derive(PartialEq, Debug)]
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

#[op]
pub fn op_url_reparse(href: String, setter: u8, setter_value: String) -> u32 {
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
      let inner_url: InnerUrl = unsafe { transmute(url) };

      // SAFETY: This is safe because we initialize URL_OFFSET_BUF in op_url_set_buf, its a null pointer
      // otherwise.
      // op_url_set_buf guarantees that the buffer is 4 * 8 bytes long.
      unsafe {
        if !URL_OFFSET_BUF.is_null() {
          let buf = std::slice::from_raw_parts_mut(URL_OFFSET_BUF, 8);
          buf[0] = inner_url.scheme_end;
          buf[1] = inner_url.username_end;
          buf[2] = inner_url.host_start;
          buf[3] = inner_url.host_end;
          buf[4] = inner_url.port.unwrap_or(0) as u32;
          buf[5] = inner_url.path_start;
          buf[6] = inner_url.query_start.unwrap_or(0);
          buf[7] = inner_url.fragment_start.unwrap_or(0);
        }
        if inner_url.serialization != href {
          URL_SERIALIZATION.replace(inner_url.serialization);
          ParseStatus::OkSerialization as u32
        } else {
          ParseStatus::Ok as u32
        }
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
