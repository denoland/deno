// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::range_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_sync;
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
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
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::usize;
use uuid::Uuid;

/// Load and execute the javascript code.
pub fn init(
  blob_url_store: BlobUrlStore,
  maybe_location: Option<Url>,
) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/web",
      "00_infra.js",
      "01_dom_exception.js",
      "01_mimesniff.js",
      "02_event.js",
      "02_structured_clone.js",
      "03_abort_signal.js",
      "04_global_interfaces.js",
      "05_base64.js",
      "08_text_encoding.js",
      "09_file.js",
      "10_filereader.js",
      "11_blob_url.js",
      "12_location.js",
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
      (
        "op_file_create_object_url",
        op_sync(op_file_create_object_url),
      ),
      (
        "op_file_revoke_object_url",
        op_sync(op_file_revoke_object_url),
      ),
    ])
    .state(move |state| {
      state.put(blob_url_store.clone());
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

  let resource = state
    .resource_table
    .get::<TextDecoderResource>(rid)
    .ok_or_else(bad_resource_id)?;

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
  let dst: &mut [u8] = &mut buffer;
  let mut read = 0;
  let mut written = 0;
  for char in input.chars() {
    let len = char.len_utf8();
    if dst.len() < written + len {
      break;
    }
    char.encode_utf8(&mut dst[written..]);
    written += len;
    if char > '\u{FFFF}' {
      read += 2
    } else {
      read += 1
    };
  }
  Ok(EncodeIntoResult { read, written })
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

#[derive(Debug, Clone)]
pub struct Blob {
  pub data: Vec<u8>,
  pub media_type: String,
}

pub struct Location(pub Url);

#[derive(Debug, Default, Clone)]
pub struct BlobUrlStore(Arc<Mutex<HashMap<Url, Blob>>>);

impl BlobUrlStore {
  pub fn get(&self, mut url: Url) -> Result<Option<Blob>, AnyError> {
    let blob_store = self.0.lock().unwrap();
    url.set_fragment(None);
    Ok(blob_store.get(&url).cloned())
  }

  pub fn insert(&self, blob: Blob, maybe_location: Option<Url>) -> Url {
    let origin = if let Some(location) = maybe_location {
      location.origin().ascii_serialization()
    } else {
      "null".to_string()
    };
    let id = Uuid::new_v4();
    let url = Url::parse(&format!("blob:{}/{}", origin, id)).unwrap();

    let mut blob_store = self.0.lock().unwrap();
    blob_store.insert(url.clone(), blob);

    url
  }

  pub fn remove(&self, url: &ModuleSpecifier) {
    let mut blob_store = self.0.lock().unwrap();
    blob_store.remove(&url);
  }
}

pub fn op_file_create_object_url(
  state: &mut deno_core::OpState,
  media_type: String,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let data = zero_copy.ok_or_else(null_opbuf)?;
  let blob = Blob {
    data: data.to_vec(),
    media_type,
  };

  let maybe_location = state.try_borrow::<Location>();
  let blob_store = state.borrow::<BlobUrlStore>();

  let url =
    blob_store.insert(blob, maybe_location.map(|location| location.0.clone()));

  Ok(url.to_string())
}

pub fn op_file_revoke_object_url(
  state: &mut deno_core::OpState,
  url: String,
  _: (),
) -> Result<(), AnyError> {
  let url = Url::parse(&url)?;
  let blob_store = state.borrow::<BlobUrlStore>();
  blob_store.remove(&url);
  Ok(())
}
