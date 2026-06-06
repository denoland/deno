// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto top-level `Crypto` interface as a cppgc-wrapped Rust object.
//!
//! Registered on the extension via `objects = [Crypto]` so the class
//! identity lives in Rust. The class is stateless on the Rust side for the
//! first migration step -- the `subtle`, `getRandomValues` and `randomUUID`
//! IDL members are still defined on the prototype from JS, where they
//! delegate to the existing `op_crypto_*` ops in `lib.rs`.

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

use crate::shared::SharedError;

pub struct Crypto;

impl WebIdlInterfaceConverter for Crypto {
  const NAME: &'static str = "Crypto";
}

// SAFETY: zero-sized payload.
unsafe impl GarbageCollected for Crypto {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static CStr {
    c"Crypto"
  }
}

#[op2]
impl Crypto {
  /// `new Crypto()` is illegal per the WebCrypto spec.
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<Crypto, SharedError> {
    Err(SharedError::IllegalConstructor)
  }
}
