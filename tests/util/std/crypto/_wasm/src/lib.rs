// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use derive_more::From;
use derive_more::Into;
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

mod digest;

/// Returns the digest of the given `data` using the given hash `algorithm`.
///
/// `length` will usually be left `undefined` to use the default length for
/// the algorithm. For algorithms with variable-length output, it can be used
/// to specify a non-negative integer number of bytes.
///
/// An error will be thrown if `algorithm` is not a supported hash algorithm or
/// `length` is not a supported length for the algorithm.
#[wasm_bindgen]
pub fn digest(
  algorithm: String,
  data: js_sys::Uint8Array,
  length: Option<usize>,
) -> Result<Box<[u8]>, JsValue> {
  let mut context = DigestContext::new(algorithm)?;
  context.update(data)?;
  context.digest_and_drop(length)
}

/// A context for incrementally computing a digest using a given hash algorithm.
#[wasm_bindgen]
#[derive(Clone, Into, From)]
pub struct DigestContext(digest::Context);

#[wasm_bindgen]
impl DigestContext {
  /// Creates a new context incrementally computing a digest using the given
  /// hash algorithm.
  ///
  /// An error will be thrown if `algorithm` is not a supported hash algorithm.
  #[wasm_bindgen(constructor)]
  pub fn new(algorithm: String) -> Result<DigestContext, JsValue> {
    Ok(
      digest::Context::new(&algorithm)
        .map_err(|message| JsValue::from(js_sys::TypeError::new(message)))?
        .into(),
    )
  }

  /// Update the digest's internal state with the additional input `data`.
  ///
  /// If the `data` array view is large, it will be split into subarrays (via
  /// JavaScript bindings) which will be processed sequentially in order to
  /// limit the amount of memory that needs to be allocated in the Wasm heap.
  #[wasm_bindgen]
  pub fn update(&mut self, data: js_sys::Uint8Array) -> Result<(), JsValue> {
    // Every method call on `data` has to go through the JavaScript bindings, so
    // try to minimize them. Splitting on the JavaScript side could be more
    // efficient, but this is called from multiple places in JavaScript so it
    // seems simpler to keep it here for now.

    let length = data.byte_length();
    let base_offset = data.byte_offset();
    let buffer = data.buffer();

    let chunk_size: u32 = 65_536;

    if length <= chunk_size {
      let chunk = data.to_vec();
      self.0.update(&chunk);
    } else {
      let mut offset = 0;
      while offset < length {
        let chunk = Uint8Array::new_with_byte_offset_and_length(
          &buffer,
          base_offset + offset,
          chunk_size.min(length - offset),
        )
        .to_vec();
        self.0.update(&chunk);
        offset += chunk_size;
      }
    }

    Ok(())
  }

  /// Returns the digest of the input data so far. This may be called repeatedly
  /// without side effects.
  ///
  /// `length` will usually be left `undefined` to use the default length for
  /// the algorithm. For algorithms with variable-length output, it can be used
  /// to specify a non-negative integer number of bytes.
  ///
  /// An error will be thrown if `algorithm` is not a supported hash algorithm or
  /// `length` is not a supported length for the algorithm.
  #[wasm_bindgen]
  pub fn digest(&self, length: Option<usize>) -> Result<Box<[u8]>, JsValue> {
    self
      .0
      .digest(length)
      .map_err(|message| JsValue::from(js_sys::TypeError::new(message)))
  }

  /// Returns the digest of the input data so far, and resets this context to
  /// its initial state, as though it has not yet been provided with any input
  /// data. (It will still use the same algorithm.)
  ///
  /// `length` will usually be left `undefined` to use the default length for
  /// the algorithm. For algorithms with variable-length output, it can be used
  /// to specify a non-negative integer number of bytes.
  ///
  /// An error will be thrown if `algorithm` is not a supported hash algorithm or
  /// `length` is not a supported length for the algorithm.
  #[wasm_bindgen(js_name=digestAndReset)]
  pub fn digest_and_reset(
    &mut self,
    length: Option<usize>,
  ) -> Result<Box<[u8]>, JsValue> {
    self
      .0
      .digest_and_reset(length)
      .map_err(|message| JsValue::from(js_sys::TypeError::new(message)))
  }

  /// Returns the digest of the input data so far, and then drops the context
  /// from memory on the Wasm side. This context must no longer be used, and any
  /// further method calls will result in null pointer errors being thrown.
  /// https://github.com/rustwasm/wasm-bindgen/blob/bf39cfd8/crates/backend/src/codegen.rs#L186
  ///
  /// `length` will usually be left `undefined` to use the default length for
  /// the algorithm. For algorithms with variable-length output, it can be used
  /// to specify a non-negative integer number of bytes.
  ///
  /// An error will be thrown if `algorithm` is not a supported hash algorithm or
  /// `length` is not a supported length for the algorithm.
  #[wasm_bindgen(js_name=digestAndDrop)]
  pub fn digest_and_drop(
    mut self,
    length: Option<usize>,
  ) -> Result<Box<[u8]>, JsValue> {
    self
      .0
      .digest_and_reset(length)
      .map_err(|message| JsValue::from(js_sys::TypeError::new(message)))
  }

  /// Resets this context to its initial state, as though it has not yet been
  /// provided with any input data. (It will still use the same algorithm.)
  #[wasm_bindgen]
  pub fn reset(&mut self) -> Result<(), JsValue> {
    self.0.reset();

    Ok(())
  }

  /// Returns a new `DigestContext` that is a copy of this one, i.e., using the
  /// same algorithm and with a copy of the same internal state.
  ///
  /// This may be a more efficient option for computing multiple digests that
  /// start with a common prefix.
  #[wasm_bindgen]
  #[allow(clippy::should_implement_trait)]
  pub fn clone(&self) -> DigestContext {
    Clone::clone(self)
  }
}
