// Copyright 2018-2026 the Deno authors. MIT license.

// Engine alias: with `--features quickjs`, the QuickJS-ng-backed compat
// crate becomes the source of truth for the `v8` crate name throughout
// serde_v8 — same alias trick deno_core uses.
#[cfg(feature = "quickjs")]
extern crate qjs_v8_compat as v8;

mod de;
mod error;
mod keys;
mod magic;
mod payload;
mod ser;

pub use de::Deserializer;
pub use de::from_v8;
pub use de::from_v8_cached;
pub use de::to_utf8;
pub use error::Error;
pub use error::Result;
pub use keys::KeyCache;
pub use magic::ExternalPointer;
pub use magic::GlobalValue;
pub use magic::Value;
pub use magic::any_value::AnyValue;
pub use magic::bigint::BigInt;
pub use magic::buffer::JsBuffer;
pub use magic::buffer::ToJsBuffer;
pub use magic::bytestring::ByteString;
pub use magic::detached_buffer::DetachedBuffer;
pub use magic::string_or_buffer::StringOrBuffer;
pub use magic::u16string::U16String;
pub use magic::v8slice::V8Slice;
pub use magic::v8slice::V8Sliceable;
pub use ser::Serializer;
pub use ser::to_v8;
