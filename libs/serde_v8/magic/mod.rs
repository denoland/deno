// Copyright 2018-2026 the Deno authors. MIT license.

pub mod any_value;
pub mod bigint;
pub mod buffer;
pub mod bytestring;
pub mod detached_buffer;
mod external_pointer;
pub mod string_or_buffer;
pub mod transl8;
pub mod u16string;
pub mod v8slice;
mod value;
pub use external_pointer::ExternalPointer;
// `Value` transmutes lifetimes off the scope and is not fully sound; it is
// kept crate-private as the deserializer's internal handle carrier and is no
// longer part of the public API.
pub(crate) use value::Value;
