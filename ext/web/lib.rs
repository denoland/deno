// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod blob;
mod message_port;

use deno_core::error::range_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use encoding_rs::CoderResult;
use encoding_rs::Decoder;
use encoding_rs::DecoderResult;
use encoding_rs::Encoding;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::usize;

use crate::blob::op_blob_create_object_url;
use crate::blob::op_blob_create_part;
use crate::blob::op_blob_read_part;
use crate::blob::op_blob_remove_part;
use crate::blob::op_blob_revoke_object_url;
use crate::blob::op_blob_slice_part;
pub use crate::blob::Blob;
pub use crate::blob::BlobPart;
pub use crate::blob::BlobStore;
pub use crate::blob::InMemoryBlobPart;

pub use crate::message_port::create_entangled_message_port;
use crate::message_port::op_message_port_create_entangled;
use crate::message_port::op_message_port_post_message;
use crate::message_port::op_message_port_recv_message;
pub use crate::message_port::JsMessageData;
pub use crate::message_port::MessagePort;

/// Load and execute the javascript code.
pub fn init(blob_store: BlobStore, maybe_location: Option<Url>) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/web",
      "00_infra.js",
      "01_dom_exception.js",
      "01_mimesniff.js",
      "02_event.js",
      "02_structured_clone.js",
      "03_abort_signal.js",
      "04_global_interfaces.js",
      "05_base64.js",
      "06_streams.js",
      "08_text_encoding.js",
      "09_file.js",
      "10_filereader.js",
      "11_blob_url.js",
      "12_location.js",
      "13_message_port.js",
    ))
    .ops(vec![
      ("op_base64_decode", op_sync(op_base64_decode)),
      ("op_base64_encode", op_sync(op_base64_encode)),
      (
        "op_encoding_normalize_label",
        op_sync(op_encoding_normalize_label),
      ),
      ("op_encoding_new_decoder", op_sync(op_encoding_new_decoder)),
      ("op_encoding_decode", op_sync(op_encoding_decode)),
      ("op_encoding_encode_into", op_sync(op_encoding_encode_into)),
      ("op_blob_create_part", op_sync(op_blob_create_part)),
      ("op_blob_slice_part", op_sync(op_blob_slice_part)),
      ("op_blob_read_part", op_async(op_blob_read_part)),
      ("op_blob_remove_part", op_sync(op_blob_remove_part)),
      (
        "op_blob_create_object_url",
        op_sync(op_blob_create_object_url),
      ),
      (
        "op_blob_revoke_object_url",
        op_sync(op_blob_revoke_object_url),
      ),
      (
        "op_message_port_create_entangled",
        op_sync(op_message_port_create_entangled),
      ),
      (
        "op_message_port_post_message",
        op_sync(op_message_port_post_message),
      ),
      (
        "op_message_port_recv_message",
        op_async(op_message_port_recv_message),
      ),
    ])
    .state(move |state| {
      state.put(blob_store.clone());
      if let Some(location) = maybe_location.clone() {
        state.put(Location(location));
      }
      Ok(())
    })
    .build()
}

fn op_base64_decode(
  _state: &mut OpState,
  input: String,
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  let mut input: &str = &input.replace(|c| char::is_ascii_whitespace(&c), "");
  // "If the length of input divides by 4 leaving no remainder, then:
  //  if input ends with one or two U+003D EQUALS SIGN (=) characters,
  //  remove them from input."
  if input.len() % 4 == 0 {
    if input.ends_with("==") {
      input = &input[..input.len() - 2]
    } else if input.ends_with('=') {
      input = &input[..input.len() - 1]
    }
  }

  // "If the length of input divides by 4 leaving a remainder of 1,
  //  throw an InvalidCharacterError exception and abort these steps."
  if input.len() % 4 == 1 {
    return Err(
      DomExceptionInvalidCharacterError::new("Failed to decode base64.").into(),
    );
  }

  if input
    .chars()
    .any(|c| c != '+' && c != '/' && !c.is_alphanumeric())
  {
    return Err(
      DomExceptionInvalidCharacterError::new(
        "Failed to decode base64: invalid character",
      )
      .into(),
    );
  }

  let cfg = base64::Config::new(base64::CharacterSet::Standard, true)
    .decode_allow_trailing_bits(true);
  let out = base64::decode_config(&input, cfg).map_err(|err| {
    DomExceptionInvalidCharacterError::new(&format!(
      "Failed to decode base64: {:?}",
      err
    ))
  })?;
  Ok(ZeroCopyBuf::from(out))
}

fn op_base64_encode(
  _state: &mut OpState,
  s: ZeroCopyBuf,
  _: (),
) -> Result<String, AnyError> {
  let cfg = base64::Config::new(base64::CharacterSet::Standard, true)
    .decode_allow_trailing_bits(true);
  let out = base64::encode_config(&s, cfg);
  Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecoderOptions {
  label: String,
  ignore_bom: bool,
  fatal: bool,
}

fn op_encoding_normalize_label(
  _state: &mut OpState,
  label: String,
  _: (),
) -> Result<String, AnyError> {
  let encoding = Encoding::for_label_no_replacement(label.as_bytes())
    .ok_or_else(|| {
      range_error(format!(
        "The encoding label provided ('{}') is invalid.",
        label
      ))
    })?;
  Ok(encoding.name().to_lowercase())
}

fn op_encoding_new_decoder(
  state: &mut OpState,
  options: DecoderOptions,
  _: (),
) -> Result<ResourceId, AnyError> {
  let DecoderOptions {
    label,
    fatal,
    ignore_bom,
  } = options;

  let encoding = Encoding::for_label(label.as_bytes()).ok_or_else(|| {
    range_error(format!(
      "The encoding label provided ('{}') is invalid.",
      label
    ))
  })?;

  let decoder = if ignore_bom {
    encoding.new_decoder_without_bom_handling()
  } else {
    encoding.new_decoder_with_bom_removal()
  };

  let rid = state.resource_table.add(TextDecoderResource {
    decoder: RefCell::new(decoder),
    fatal,
  });

  Ok(rid)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecodeOptions {
  rid: ResourceId,
  stream: bool,
}

fn op_encoding_decode(
  state: &mut OpState,
  data: ZeroCopyBuf,
  options: DecodeOptions,
) -> Result<String, AnyError> {
  let DecodeOptions { rid, stream } = options;

  let resource = state.resource_table.get::<TextDecoderResource>(rid)?;

  let mut decoder = resource.decoder.borrow_mut();
  let fatal = resource.fatal;

  let max_buffer_length = if fatal {
    decoder
      .max_utf8_buffer_length_without_replacement(data.len())
      .ok_or_else(|| range_error("Value too large to decode."))?
  } else {
    decoder
      .max_utf8_buffer_length(data.len())
      .ok_or_else(|| range_error("Value too large to decode."))?
  };

  let mut output = String::with_capacity(max_buffer_length);

  if fatal {
    let (result, _) =
      decoder.decode_to_string_without_replacement(&data, &mut output, !stream);
    match result {
      DecoderResult::InputEmpty => Ok(output),
      DecoderResult::OutputFull => {
        Err(range_error("Provided buffer too small."))
      }
      DecoderResult::Malformed(_, _) => {
        Err(type_error("The encoded data is not valid."))
      }
    }
  } else {
    let (result, _, _) = decoder.decode_to_string(&data, &mut output, !stream);
    match result {
      CoderResult::InputEmpty => Ok(output),
      CoderResult::OutputFull => Err(range_error("Provided buffer too small.")),
    }
  }
}

struct TextDecoderResource {
  decoder: RefCell<Decoder>,
  fatal: bool,
}

impl Resource for TextDecoderResource {
  fn name(&self) -> Cow<str> {
    "textDecoder".into()
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EncodeIntoResult {
  read: usize,
  written: usize,
}

fn op_encoding_encode_into(
  _state: &mut OpState,
  input: String,
  mut buffer: ZeroCopyBuf,
) -> Result<EncodeIntoResult, AnyError> {
  // Since `input` is already UTF-8, we can simply find the last UTF-8 code
  // point boundary from input that fits in `buffer`, and copy the bytes up to
  // that point.
  let boundary = if buffer.len() >= input.len() {
    input.len()
  } else {
    let mut boundary = buffer.len();

    // The maximum length of a UTF-8 code point is 4 bytes.
    for _ in 0..4 {
      if input.is_char_boundary(boundary) {
        break;
      }
      debug_assert!(boundary > 0);
      boundary -= 1;
    }

    debug_assert!(input.is_char_boundary(boundary));
    boundary
  };

  buffer[..boundary].copy_from_slice(input[..boundary].as_bytes());

  Ok(EncodeIntoResult {
    // The `read` output parameter is measured in UTF-16 code units.
    read: input[..boundary].encode_utf16().count(),
    written: boundary,
  })
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_web.d.ts")
}

#[derive(Debug)]
pub struct DomExceptionQuotaExceededError {
  pub msg: String,
}

impl DomExceptionQuotaExceededError {
  pub fn new(msg: &str) -> Self {
    DomExceptionQuotaExceededError {
      msg: msg.to_string(),
    }
  }
}

#[derive(Debug)]
pub struct DomExceptionInvalidCharacterError {
  pub msg: String,
}

impl DomExceptionInvalidCharacterError {
  pub fn new(msg: &str) -> Self {
    DomExceptionInvalidCharacterError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionQuotaExceededError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}
impl fmt::Display for DomExceptionInvalidCharacterError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionQuotaExceededError {}

impl std::error::Error for DomExceptionInvalidCharacterError {}

pub fn get_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionQuotaExceededError>()
    .map(|_| "DOMExceptionQuotaExceededError")
    .or_else(|| {
      e.downcast_ref::<DomExceptionInvalidCharacterError>()
        .map(|_| "DOMExceptionInvalidCharacterError")
    })
}
pub struct Location(pub Url);
