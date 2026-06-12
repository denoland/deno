// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto top-level `Crypto` interface as a cppgc-wrapped Rust object.
//!
//! Registered on the extension via `objects = [Crypto]` so the class
//! identity lives in Rust. `getRandomValues`, `randomUUID` and the `subtle`
//! getter are implemented natively as `#[op2] impl` members; the JS shim
//! only constructs the singleton via the [`Crypto::create`] static method.

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;
use rand::Rng;
use rand::rngs::StdRng;
use rand::thread_rng;

use crate::CryptoError;
use crate::fast_uuid_v4;
use crate::shared::SharedError;

pub struct Crypto {
  /// The single `SubtleCrypto` instance returned by the `subtle` getter.
  /// Stored as a `v8::Global` so the getter returns the same identity
  /// across calls, as required by Web IDL.
  subtle: v8::Global<v8::Value>,
}

impl WebIdlInterfaceConverter for Crypto {
  const NAME: &'static str = "Crypto";
}

// SAFETY: the `subtle` field is a `v8::Global` whose backing object is owned
// by V8 itself; no Rust-side roots that need cppgc tracing.
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

  /// Mint the singleton `Crypto` instance for `globalThis.crypto`. The JS
  /// shim passes the already-constructed `SubtleCrypto` cppgc object so
  /// that the `subtle` getter returns the same identity every call. Stays
  /// as a static method on the class (not a top-level op) so it travels
  /// with the cppgc class definition.
  #[required(1)]
  #[static_method]
  #[cppgc]
  fn create(
    scope: &mut v8::PinScope<'_, '_>,
    subtle: v8::Local<v8::Value>,
  ) -> Crypto {
    Crypto {
      subtle: v8::Global::new(scope, subtle),
    }
  }

  #[getter]
  fn subtle<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Value> {
    v8::Local::new(scope, &self.subtle)
  }

  /// `Crypto.getRandomValues(typedArray)` — fills `typedArray` with
  /// cryptographically strong random bytes and returns it unchanged.
  /// Rejects non-`ArrayBufferView` arguments with `TypeError` (per WebIDL
  /// `ArrayBufferView` conversion), non-integer typed-array kinds with
  /// `TypeMismatchError`, and inputs longer than 65536 bytes with
  /// `QuotaExceededError` (both per the WebCrypto spec).
  #[required(1)]
  fn get_random_values<'s>(
    &self,
    state: &mut OpState,
    scope: &mut v8::PinScope<'s, '_>,
    typed_array: v8::Local<'s, v8::Value>,
  ) -> Result<v8::Local<'s, v8::Value>, CryptoError> {
    let view = v8::Local::<v8::ArrayBufferView>::try_from(typed_array)
      .map_err(|_| CryptoError::ArgumentNotArrayBufferView)?;
    if !(view.is_int8_array()
      || view.is_uint8_array()
      || view.is_uint8_clamped_array()
      || view.is_int16_array()
      || view.is_uint16_array()
      || view.is_int32_array()
      || view.is_uint32_array()
      || view.is_big_int64_array()
      || view.is_big_uint64_array())
    {
      return Err(CryptoError::TypedArrayNotInteger);
    }

    let byte_len = view.byte_length();
    if byte_len > 65536 {
      return Err(CryptoError::ArrayBufferViewLengthExceeded(byte_len));
    }
    if byte_len > 0 {
      let byte_offset = view.byte_offset();
      let ab = view.buffer(scope).unwrap();
      // SAFETY: byte_offset + byte_len are within the backing store per V8.
      let bytes = unsafe {
        let ptr = (ab.data().unwrap().as_ptr() as *mut u8).add(byte_offset);
        std::slice::from_raw_parts_mut(ptr, byte_len)
      };

      let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
      if let Some(seeded_rng) = maybe_seeded_rng {
        seeded_rng.fill(bytes);
      } else {
        let mut rng = thread_rng();
        rng.fill(bytes);
      }
    }
    Ok(typed_array)
  }

  /// `Crypto.registerSymbols(webidlBrand, kKeyObject)` — internal static
  /// method called by the crypto module bootstrap to hand the WebIDL brand
  /// symbol (a private of `ext:deno_webidl/00_webidl.js`) and the
  /// `kKeyObject` symbol (a private of ext/node) over to the crypto cppgc
  /// methods, which need them to brand every freshly-constructed
  /// `CryptoKey`. Idempotent and effect-only; returns `true` on success.
  #[fast]
  #[rename("registerSymbols")]
  #[static_method]
  fn register_symbols<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    webidl_brand: v8::Local<'s, v8::Value>,
    k_key_object: v8::Local<'s, v8::Value>,
  ) -> bool {
    crate::make_key::register_symbols(scope, webidl_brand, k_key_object)
  }

  #[string]
  #[rename("randomUUID")]
  #[required(0)]
  fn random_uuid(&self, state: &mut OpState) -> String {
    let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
    let mut bytes = [0u8; 16];
    if let Some(seeded_rng) = maybe_seeded_rng {
      seeded_rng.fill(&mut bytes);
    } else {
      let mut rng = thread_rng();
      rng.fill(&mut bytes);
    }
    fast_uuid_v4(&mut bytes)
  }
}
