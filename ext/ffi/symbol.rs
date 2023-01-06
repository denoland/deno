// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use std::alloc::Layout;

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

impl NativeType {
  pub fn get_size(&self) -> Result<usize, AnyError> {
    Ok(match self {
      NativeType::Void => 0,
      NativeType::U8 | NativeType::I8 | NativeType::Bool => 1,
      NativeType::U16 | NativeType::I16 => 2,
      NativeType::U32 | NativeType::I32 | NativeType::F32 => 4,
      NativeType::U64 | NativeType::I64 | NativeType::F64 => 8,
      NativeType::USize
      | NativeType::ISize
      | NativeType::Pointer
      | NativeType::Function
      | NativeType::Buffer => std::mem::size_of::<usize>(),
      NativeType::Struct(_) => self.as_layout()?.size(),
    })
  }

  pub fn as_layout(&self) -> Result<Layout, AnyError> {
    Ok(match self {
      NativeType::Void => {
        return Err(type_error("Void type cannot be used as a struct field"))
      }
      NativeType::Bool => Layout::new::<bool>(),
      NativeType::U8 => Layout::new::<u8>(),
      NativeType::I8 => Layout::new::<i8>(),
      NativeType::U16 => Layout::new::<u16>(),
      NativeType::I16 => Layout::new::<i16>(),
      NativeType::U32 => Layout::new::<u32>(),
      NativeType::I32 => Layout::new::<i32>(),
      NativeType::U64 => Layout::new::<u64>(),
      NativeType::I64 => Layout::new::<i64>(),
      NativeType::USize => Layout::new::<usize>(),
      NativeType::ISize => Layout::new::<isize>(),
      NativeType::F32 => Layout::new::<f32>(),
      NativeType::F64 => Layout::new::<f64>(),
      NativeType::Pointer => Layout::new::<usize>(),
      NativeType::Buffer => Layout::new::<usize>(),
      NativeType::Function => Layout::new::<usize>(),
      NativeType::Struct(fields) => {
        let mut layout = Layout::from_size_align(0, 1)?;
        for field in fields.iter() {
          let (new_layout, _) = layout.extend(field.as_layout()?)?;
          layout = new_layout;
        }
        layout.pad_to_align()
      }
    })
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
