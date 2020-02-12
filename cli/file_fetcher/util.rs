// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::msg;
use regex::Regex;
use serde_json;
use std;
use std::path::Path;
use std::path::PathBuf;
use std::result::Result;
use std::str;
use url;
use url::Url;

pub fn map_file_extension(path: &Path) -> msg::MediaType {
  match path.extension() {
    None => msg::MediaType::Unknown,
    Some(os_str) => match os_str.to_str() {
      Some("ts") => msg::MediaType::TypeScript,
      Some("tsx") => msg::MediaType::TSX,
      Some("js") => msg::MediaType::JavaScript,
      Some("jsx") => msg::MediaType::JSX,
      Some("mjs") => msg::MediaType::JavaScript,
      Some("json") => msg::MediaType::Json,
      Some("wasm") => msg::MediaType::Wasm,
      _ => msg::MediaType::Unknown,
    },
  }
}

// convert a ContentType string into a enumerated MediaType
pub fn map_content_type(
  path: &Path,
  content_type: Option<&str>,
) -> msg::MediaType {
  match content_type {
    Some(content_type) => {
      // sometimes there is additional data after the media type in
      // Content-Type so we have to do a bit of manipulation so we are only
      // dealing with the actual media type
      let ct_vector: Vec<&str> = content_type.split(';').collect();
      let ct: &str = ct_vector.first().unwrap();
      match ct.to_lowercase().as_ref() {
        "application/typescript"
        | "text/typescript"
        | "video/vnd.dlna.mpeg-tts"
        | "video/mp2t"
        | "application/x-typescript" => {
          map_js_like_extension(path, msg::MediaType::TypeScript)
        }
        "application/javascript"
        | "text/javascript"
        | "application/ecmascript"
        | "text/ecmascript"
        | "application/x-javascript" => {
          map_js_like_extension(path, msg::MediaType::JavaScript)
        }
        "application/json" | "text/json" => msg::MediaType::Json,
        // Handle plain and possibly webassembly
        "text/plain" | "application/octet-stream" => map_file_extension(path),
        _ => {
          debug!("unknown content type: {}", content_type);
          msg::MediaType::Unknown
        }
      }
    }
    None => map_file_extension(path),
  }
}

pub fn map_js_like_extension(
  path: &Path,
  default: msg::MediaType,
) -> msg::MediaType {
  match path.extension() {
    None => default,
    Some(os_str) => match os_str.to_str() {
      None => default,
      Some("jsx") => msg::MediaType::JSX,
      Some("tsx") => msg::MediaType::TSX,
      Some(_) => default,
    },
  }
}

/// Take a module URL and source code and determines if the source code contains
/// a type directive, and if so, returns the parsed URL for that type directive.
pub fn get_types_url(
  module_url: &Url,
  source_code: &[u8],
  maybe_types_header: Option<&str>,
) -> Option<Url> {
  lazy_static! {
    /// Matches reference type directives in strings, which provide
    /// type files that should be used by the compiler instead of the
    /// JavaScript file.
    static ref DIRECTIVE_TYPES: Regex = Regex::new(
      r#"(?m)^/{3}\s*<reference\s+types\s*=\s*["']([^"']+)["']\s*/>"#
    )
    .unwrap();
  }

  match maybe_types_header {
    Some(types_header) => match Url::parse(&types_header) {
      Ok(url) => Some(url),
      _ => Some(module_url.join(&types_header).unwrap()),
    },
    _ => match DIRECTIVE_TYPES.captures(str::from_utf8(source_code).unwrap()) {
      Some(cap) => {
        let val = cap.get(1).unwrap().as_str();
        match Url::parse(&val) {
          Ok(url) => Some(url),
          _ => Some(module_url.join(&val).unwrap()),
        }
      }
      _ => None,
    },
  }
}

pub fn filter_shebang(bytes: Vec<u8>) -> Vec<u8> {
  let string = str::from_utf8(&bytes).unwrap();
  if let Some(i) = string.find('\n') {
    let (_, rest) = string.split_at(i);
    rest.as_bytes().to_owned()
  } else {
    Vec::new()
  }
}

pub fn check_cache_blacklist(url: &Url, black_list: &[String]) -> bool {
  let mut url_without_fragmets = url.clone();
  url_without_fragmets.set_fragment(None);
  if black_list.contains(&String::from(url_without_fragmets.as_str())) {
    return true;
  }
  let mut url_without_query_strings = url_without_fragmets;
  url_without_query_strings.set_query(None);
  let mut path_buf = PathBuf::from(url_without_query_strings.as_str());
  loop {
    if black_list.contains(&String::from(path_buf.to_str().unwrap())) {
      return true;
    }
    if !path_buf.pop() {
      break;
    }
  }
  false
}

#[derive(Debug, Default)]
/// Header metadata associated with a particular "symbolic" source code file.
/// (the associated source code file might not be cached, while remaining
/// a user accessible entity through imports (due to redirects)).
pub struct SourceCodeHeaders {
  /// MIME type of the source code.
  pub mime_type: Option<String>,
  /// Where should we actually look for source code.
  /// This should be an absolute path!
  pub redirect_to: Option<String>,
  /// ETag of the remote source file
  pub etag: Option<String>,
  /// X-TypeScript-Types defines the location of a .d.ts file
  pub x_typescript_types: Option<String>,
}

static MIME_TYPE: &str = "mime_type";
static REDIRECT_TO: &str = "redirect_to";
static ETAG: &str = "etag";
static X_TYPESCRIPT_TYPES: &str = "x_typescript_types";

impl SourceCodeHeaders {
  pub fn from_json_string(headers_string: String) -> Self {
    // TODO: use serde for deserialization
    let maybe_headers_json: serde_json::Result<serde_json::Value> =
      serde_json::from_str(&headers_string);

    if let Ok(headers_json) = maybe_headers_json {
      let mime_type = headers_json[MIME_TYPE].as_str().map(String::from);
      let redirect_to = headers_json[REDIRECT_TO].as_str().map(String::from);
      let etag = headers_json[ETAG].as_str().map(String::from);
      let x_typescript_types =
        headers_json[X_TYPESCRIPT_TYPES].as_str().map(String::from);

      return SourceCodeHeaders {
        mime_type,
        redirect_to,
        etag,
        x_typescript_types,
      };
    }

    SourceCodeHeaders::default()
  }

  // TODO: remove this nonsense `cache_filename` param, this should be
  //  done when instantiating SourceCodeHeaders
  pub fn to_json_string(
    &self,
    cache_filename: &Path,
  ) -> Result<Option<String>, serde_json::Error> {
    // TODO(kevinkassimo): consider introduce serde::Deserialize to make things simpler.
    // This is super ugly at this moment...
    // Had trouble to make serde_derive work: I'm unable to build proc-macro2.
    let mut value_map = serde_json::map::Map::new();

    if let Some(mime_type) = &self.mime_type {
      let resolved_mime_type =
        map_content_type(Path::new(""), Some(mime_type.clone().as_str()));

      // TODO: fix this
      let ext_based_mime_type = map_file_extension(cache_filename);

      // Add mime to headers only when content type is different from extension.
      if ext_based_mime_type == msg::MediaType::Unknown
        || resolved_mime_type != ext_based_mime_type
      {
        value_map.insert(MIME_TYPE.to_string(), json!(mime_type));
      }
    }

    if let Some(redirect_to) = &self.redirect_to {
      value_map.insert(REDIRECT_TO.to_string(), json!(redirect_to));
    }

    if let Some(etag) = &self.etag {
      value_map.insert(ETAG.to_string(), json!(etag));
    }

    if let Some(x_typescript_types) = &self.x_typescript_types {
      value_map
        .insert(X_TYPESCRIPT_TYPES.to_string(), json!(x_typescript_types));
    }

    if value_map.is_empty() {
      return Ok(None);
    }

    serde_json::to_string(&value_map)
      .and_then(|serialized| Ok(Some(serialized)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_cache_blacklist() {
    let args = crate::flags::resolve_urls(vec![
      String::from("http://deno.land/std"),
      String::from("http://github.com/example/mod.ts"),
      String::from("http://fragment.com/mod.ts#fragment"),
      String::from("http://query.com/mod.ts?foo=bar"),
      String::from("http://queryandfragment.com/mod.ts?foo=bar#fragment"),
    ]);

    let u: Url = "http://deno.land/std/fs/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://github.com/example/file.ts".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), false);

    let u: Url = "http://github.com/example/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://github.com/example/mod.ts?foo=bar".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://github.com/example/mod.ts#fragment".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://fragment.com/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://query.com/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), false);

    let u: Url = "http://fragment.com/mod.ts#fragment".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://query.com/mod.ts?foo=bar".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://queryandfragment.com/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), false);

    let u: Url = "http://queryandfragment.com/mod.ts?foo=bar"
      .parse()
      .unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://queryandfragment.com/mod.ts#fragment"
      .parse()
      .unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), false);

    let u: Url = "http://query.com/mod.ts?foo=bar#fragment".parse().unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);

    let u: Url = "http://fragment.com/mod.ts?foo=bar#fragment"
      .parse()
      .unwrap();
    assert_eq!(check_cache_blacklist(&u, &args), true);
  }
  #[test]
  fn test_map_file_extension() {
    assert_eq!(
      map_file_extension(Path::new("foo/bar.ts")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.tsx")),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.d.ts")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.js")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.jsx")),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.json")),
      msg::MediaType::Json
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.wasm")),
      msg::MediaType::Wasm
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.txt")),
      msg::MediaType::Unknown
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar")),
      msg::MediaType::Unknown
    );
  }

  #[test]
  fn test_map_content_type_extension_only() {
    // Extension only
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), None),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.tsx"), None),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.d.ts"), None),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.js"), None),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.txt"), None),
      msg::MediaType::Unknown
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.jsx"), None),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.json"), None),
      msg::MediaType::Json
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.wasm"), None),
      msg::MediaType::Wasm
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), None),
      msg::MediaType::Unknown
    );
  }

  #[test]
  fn test_map_content_type_media_type_with_no_extension() {
    // Media Type
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/typescript")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/typescript")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("video/vnd.dlna.mpeg-tts")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("video/mp2t")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/x-typescript")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/javascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/javascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/ecmascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/ecmascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/x-javascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/json")),
      msg::MediaType::Json
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/json")),
      msg::MediaType::Json
    );
  }

  #[test]
  fn test_map_file_extension_media_type_with_extension() {
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), Some("text/plain")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), Some("foo/bar")),
      msg::MediaType::Unknown
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("application/typescript"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("application/javascript"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("application/x-typescript"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("video/vnd.dlna.mpeg-tts"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.tsx"), Some("video/mp2t")),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/javascript"),
      ),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/x-typescript"),
      ),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/ecmascript"),
      ),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.jsx"), Some("text/ecmascript")),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/x-javascript"),
      ),
      msg::MediaType::JSX
    );
  }

  #[test]
  fn test_filter_shebang() {
    assert_eq!(filter_shebang(b"#!"[..].to_owned()), b"");
    assert_eq!(filter_shebang(b"#!\n\n"[..].to_owned()), b"\n\n");
    let code = b"#!/usr/bin/env deno\nconsole.log('hello');\n"[..].to_owned();
    assert_eq!(filter_shebang(code), b"\nconsole.log('hello');\n");
  }

  #[test]
  fn test_get_types_url_1() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = b"console.log(\"foo\");".to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(result, None);
  }

  #[test]
  fn test_get_types_url_2() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = r#"/// <reference types="./mod.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(
      result,
      Some(Url::parse("https://example.com/mod.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_3() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = r#"/// <reference types="https://deno.land/mod.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(
      result,
      Some(Url::parse("https://deno.land/mod.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_4() {
    let module_url = Url::parse("file:///foo/bar/baz.js").unwrap();
    let source_code = r#"/// <reference types="../qat/baz.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(
      result,
      Some(Url::parse("file:///foo/qat/baz.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_5() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = b"console.log(\"foo\");".to_owned();
    let result = get_types_url(&module_url, &source_code, Some("./mod.d.ts"));
    assert_eq!(
      result,
      Some(Url::parse("https://example.com/mod.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_6() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = r#"/// <reference types="./mod.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(
      &module_url,
      &source_code,
      Some("https://deno.land/mod.d.ts"),
    );
    assert_eq!(
      result,
      Some(Url::parse("https://deno.land/mod.d.ts").unwrap())
    );
  }
}
