// Copyright 2018-2025 the Deno authors. MIT license.

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
