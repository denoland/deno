// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/// Defines the accepted types that can be used as
/// parameters and return values in FFI.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NativeType {
  Void,
  Bool,
  U8,
  I8,
  U16,
  I16,
  U32,
  I32,
  U64,
  I64,
  USize,
  ISize,
  F32,
  F64,
  Pointer,
  Buffer,
  Function,
  Struct(Box<[NativeType]>),
}

impl From<NativeType> for libffi::middle::Type {
  fn from(native_type: NativeType) -> Self {
    match native_type {
      NativeType::Void => libffi::middle::Type::void(),
      NativeType::U8 | NativeType::Bool => libffi::middle::Type::u8(),
      NativeType::I8 => libffi::middle::Type::i8(),
      NativeType::U16 => libffi::middle::Type::u16(),
      NativeType::I16 => libffi::middle::Type::i16(),
      NativeType::U32 => libffi::middle::Type::u32(),
      NativeType::I32 => libffi::middle::Type::i32(),
      NativeType::U64 => libffi::middle::Type::u64(),
      NativeType::I64 => libffi::middle::Type::i64(),
      NativeType::USize => libffi::middle::Type::usize(),
      NativeType::ISize => libffi::middle::Type::isize(),
      NativeType::F32 => libffi::middle::Type::f32(),
      NativeType::F64 => libffi::middle::Type::f64(),
      NativeType::Pointer | NativeType::Buffer | NativeType::Function => {
        libffi::middle::Type::pointer()
      }
      NativeType::Struct(fields) => libffi::middle::Type::structure(
        fields.iter().map(|field| field.clone().into()),
      ),
    }
  }
}

#[derive(Clone)]
pub struct Symbol {
  pub cif: libffi::middle::Cif,
  pub ptr: libffi::middle::CodePtr,
  pub parameter_types: Vec<NativeType>,
  pub result_type: NativeType,
  pub can_callback: bool,
}

#[allow(clippy::non_send_fields_in_send_ty)]
// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Send for Symbol {}
// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Sync for Symbol {}
