// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto `CryptoKey` as a cppgc-wrapped Rust object.
//!
//! `CryptoKey` is registered on the `deno_crypto` extension via
//! `objects = [CryptoKey]` so the class identity (the constructor function and
//! its `.prototype`) lives in Rust. The JavaScript shim that used to declare
//! `class CryptoKey { ... }` in `00_crypto.js` is gone -- the shim now imports
//! this constructor from `ext:core/ops` and decorates its prototype with the
//! existing getter/inspect machinery while we lift each WebCrypto operation
//! onto native code.
//!
//! The instance is intentionally stateless on the Rust side for the first
//! migration step. The original `[[type]]` / `[[extractable]]` / `[[usages]]`
//! / `[[algorithm]]` / `[[handle]]` private slots are still set by JS as
//! private-symbol properties on each branded instance. Subsequent sessions
//! lift the slots onto the Rust struct and replace the symbol reads with
//! `#[getter]` methods declared on this `#[op2] impl` block.

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

use crate::shared::SharedError;

pub struct CryptoKey;

impl WebIdlInterfaceConverter for CryptoKey {
  const NAME: &'static str = "CryptoKey";
}

// SAFETY: zero-sized payload.
unsafe impl GarbageCollected for CryptoKey {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static CStr {
    c"CryptoKey"
  }
}

#[op2]
impl CryptoKey {
  /// `new CryptoKey()` is an illegal-constructor per the WebCrypto spec.
  /// Branded instances are produced via `webidl.createBranded(CryptoKey)`
  /// in the JS shim, which does not invoke the constructor.
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<CryptoKey, SharedError> {
    Err(SharedError::IllegalConstructor)
  }
}
