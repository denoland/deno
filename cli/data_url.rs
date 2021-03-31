// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::media_type::MediaType;
use crate::text_encoding;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use log::debug;
use std::path::PathBuf;

/// Given a vector of bytes and optionally a charset, decode the bytes to a
/// string.
pub fn get_source_from_bytes(
  bytes: Vec<u8>,
  maybe_charset: Option<String>,
) -> Result<String, AnyError> {
  let source = if let Some(charset) = maybe_charset {
    text_encoding::convert_to_utf8(&bytes, &charset)?.to_string()
  } else {
    String::from_utf8(bytes)?
  };

  Ok(source)
}

pub fn get_source_from_data_url(
  specifier: &ModuleSpecifier,
) -> Result<(String, MediaType, String), AnyError> {
  if specifier.scheme() != "data" {
    return Err(custom_error(
      "BadScheme",
      format!("Unexpected scheme of \"{}\"", specifier.scheme()),
    ));
  }
  let path = specifier.path();
  let mut parts = path.splitn(2, ',');
  let media_type_part =
    percent_encoding::percent_decode_str(parts.next().unwrap())
      .decode_utf8()?;
  let data_part = if let Some(data) = parts.next() {
    data
  } else {
    return Err(custom_error(
      "BadUrl",
      "The data URL is badly formed, missing a comma.",
    ));
  };
  let (media_type, maybe_charset) =
    map_content_type(specifier, Some(media_type_part.to_string()));
  let is_base64 = media_type_part.rsplit(';').any(|p| p == "base64");
  let bytes = if is_base64 {
    base64::decode(data_part)?
  } else {
    percent_encoding::percent_decode_str(data_part).collect()
  };
  let source = strip_shebang(get_source_from_bytes(bytes, maybe_charset)?);
  Ok((source, media_type, media_type_part.to_string()))
}

/// Remove shebangs from the start of source code strings
pub fn strip_shebang(mut value: String) -> String {
  if value.starts_with("#!") {
    if let Some(mid) = value.find('\n') {
      let (_, rest) = value.split_at(mid);
      value = rest.to_string()
    } else {
      value.clear()
    }
  }
  value
}

/// Resolve a media type and optionally the charset from a module specifier and
/// the value of a content type header.
pub fn map_content_type(
  specifier: &ModuleSpecifier,
  maybe_content_type: Option<String>,
) -> (MediaType, Option<String>) {
  if let Some(content_type) = maybe_content_type {
    let mut content_types = content_type.split(';');
    let content_type = content_types.next().unwrap();
    let media_type = match content_type.trim().to_lowercase().as_ref() {
      "application/typescript"
      | "text/typescript"
      | "video/vnd.dlna.mpeg-tts"
      | "video/mp2t"
      | "application/x-typescript" => {
        map_js_like_extension(specifier, MediaType::TypeScript)
      }
      "application/javascript"
      | "text/javascript"
      | "application/ecmascript"
      | "text/ecmascript"
      | "application/x-javascript"
      | "application/node" => {
        map_js_like_extension(specifier, MediaType::JavaScript)
      }
      "text/jsx" => MediaType::Jsx,
      "text/tsx" => MediaType::Tsx,
      "application/json" | "text/json" => MediaType::Json,
      "application/wasm" => MediaType::Wasm,
      // Handle plain and possibly webassembly
      "text/plain" | "application/octet-stream" => MediaType::from(specifier),
      _ => {
        debug!("unknown content type: {}", content_type);
        MediaType::Unknown
      }
    };
    let charset = content_types
      .map(str::trim)
      .find_map(|s| s.strip_prefix("charset="))
      .map(String::from);

    (media_type, charset)
  } else {
    (MediaType::from(specifier), None)
  }
}

/// Used to augment media types by using the path part of a module specifier to
/// resolve to a more accurate media type.
pub fn map_js_like_extension(
  specifier: &ModuleSpecifier,
  default: MediaType,
) -> MediaType {
  let path = if specifier.scheme() == "file" {
    if let Ok(path) = specifier.to_file_path() {
      path
    } else {
      PathBuf::from(specifier.path())
    }
  } else {
    PathBuf::from(specifier.path())
  };
  match path.extension() {
    None => default,
    Some(os_str) => match os_str.to_str() {
      None => default,
      Some("jsx") => MediaType::Jsx,
      Some("tsx") => MediaType::Tsx,
      // Because DTS files do not have a separate media type, or a unique
      // extension, we have to "guess" at those things that we consider that
      // look like TypeScript, and end with `.d.ts` are DTS files.
      Some("ts") => {
        if default == MediaType::TypeScript {
          match path.file_stem() {
            None => default,
            Some(os_str) => {
              if let Some(file_stem) = os_str.to_str() {
                if file_stem.ends_with(".d") {
                  MediaType::Dts
                } else {
                  default
                }
              } else {
                default
              }
            }
          }
        } else {
          default
        }
      }
      Some(_) => default,
    },
  }
}
