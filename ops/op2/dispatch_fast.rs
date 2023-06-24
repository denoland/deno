// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use super::generator_state::GeneratorState;
use super::signature::Arg;
use super::signature::NumericArg;
use super::signature::ParsedSignature;
use super::signature::RetVal;
use super::V8MappingError;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;

#[allow(unused)]
#[derive(Debug, Default, PartialEq, Clone)]
pub(crate) enum FastValue {
  #[default]
  Void,
  Bool,
  U32,
  I32,
  U64,
  I64,
  F32,
  F64,
  Pointer,
  V8Value,
  Uint8Array,
  Uint32Array,
  Float64Array,
  SeqOneByteString,
}

impl FastValue {
  /// Quote fast value type.
  fn quote_rust_type(&self) -> TokenStream {
    match self {
      FastValue::Void => quote!(()),
      FastValue::Bool => quote!(bool),
      FastValue::U32 => quote!(u32),
      FastValue::I32 => quote!(i32),
      FastValue::U64 => quote!(u64),
      FastValue::I64 => quote!(i64),
      FastValue::F32 => quote!(f32),
      FastValue::F64 => quote!(f64),
      FastValue::Pointer => quote!(*mut ::std::ffi::c_void),
      FastValue::V8Value => unimplemented!("v8::Local<v8::Value>"),
      FastValue::Uint8Array
      | FastValue::Uint32Array
      | FastValue::Float64Array
      | FastValue::SeqOneByteString => unreachable!(),
    }
  }

  /// Quote fast value type's variant.
  fn quote_ctype(&self) -> TokenStream {
    match &self {
      FastValue::Void => quote!(CType::Void),
      FastValue::Bool => quote!(CType::Bool),
      FastValue::U32 => quote!(CType::Uint32),
      FastValue::I32 => quote!(CType::Int32),
      FastValue::U64 => quote!(CType::Uint64),
      FastValue::I64 => quote!(CType::Int64),
      FastValue::F32 => quote!(CType::Float32),
      FastValue::F64 => quote!(CType::Float64),
      FastValue::Pointer => quote!(CType::Pointer),
      FastValue::V8Value => quote!(CType::V8Value),
      FastValue::Uint8Array => unreachable!(),
      FastValue::Uint32Array => unreachable!(),
      FastValue::Float64Array => unreachable!(),
      FastValue::SeqOneByteString => quote!(CType::SeqOneByteString),
    }
  }

  /// Quote fast value type's variant.
  fn quote_type(&self) -> TokenStream {
    match &self {
      FastValue::Void => quote!(Type::Void),
      FastValue::Bool => quote!(Type::Bool),
      FastValue::U32 => quote!(Type::Uint32),
      FastValue::I32 => quote!(Type::Int32),
      FastValue::U64 => quote!(Type::Uint64),
      FastValue::I64 => quote!(Type::Int64),
      FastValue::F32 => quote!(Type::Float32),
      FastValue::F64 => quote!(Type::Float64),
      FastValue::Pointer => quote!(Type::Pointer),
      FastValue::V8Value => quote!(Type::V8Value),
      FastValue::Uint8Array => quote!(Type::TypedArray(CType::Uint8)),
      FastValue::Uint32Array => quote!(Type::TypedArray(CType::Uint32)),
      FastValue::Float64Array => quote!(Type::TypedArray(CType::Float64)),
      FastValue::SeqOneByteString => quote!(Type::SeqOneByteString),
    }
  }
}

pub fn generate_dispatch_fast(
  generator_state: &mut GeneratorState,
  signature: &ParsedSignature,
) -> Result<Option<(TokenStream, TokenStream)>, V8MappingError> {
  let mut inputs = vec![];
  for arg in &signature.args {
    let fv = match arg {
      Arg::OptionNumeric(_) | Arg::SerdeV8(_) => return Ok(None),
      Arg::Numeric(NumericArg::bool) => FastValue::Bool,
      Arg::Numeric(NumericArg::u32)
      | Arg::Numeric(NumericArg::u16)
      | Arg::Numeric(NumericArg::u8) => FastValue::U32,
      Arg::Numeric(NumericArg::i32)
      | Arg::Numeric(NumericArg::i16)
      | Arg::Numeric(NumericArg::i8)
      | Arg::Numeric(NumericArg::__SMI__) => FastValue::I32,
      Arg::Numeric(NumericArg::u64) | Arg::Numeric(NumericArg::usize) => {
        FastValue::U64
      }
      Arg::Numeric(NumericArg::i64) | Arg::Numeric(NumericArg::isize) => {
        FastValue::I64
      }
      _ => {
        return Err(V8MappingError::NoMapping("a fast argument", arg.clone()))
      }
    };
    inputs.push(fv);
  }

  let ret_val = match &signature.ret_val {
    RetVal::Infallible(arg) => arg,
    RetVal::Result(arg) => arg,
  };

  let output = match ret_val {
    Arg::OptionNumeric(_) | Arg::SerdeV8(_) => return Ok(None),
    Arg::Void => FastValue::Void,
    Arg::Numeric(NumericArg::bool) => FastValue::Bool,
    Arg::Numeric(NumericArg::u32)
    | Arg::Numeric(NumericArg::u16)
    | Arg::Numeric(NumericArg::u8) => FastValue::U32,
    Arg::Numeric(NumericArg::i32)
    | Arg::Numeric(NumericArg::i16)
    | Arg::Numeric(NumericArg::i8) => FastValue::I32,
    Arg::Numeric(NumericArg::u64) | Arg::Numeric(NumericArg::usize) => {
      FastValue::U64
    }
    Arg::Numeric(NumericArg::i64) | Arg::Numeric(NumericArg::isize) => {
      FastValue::I64
    }
    Arg::Special(_) => return Ok(None),
    _ => {
      return Err(V8MappingError::NoMapping(
        "a fast return value",
        ret_val.clone(),
      ))
    }
  };

  let GeneratorState {
    fast_function,
    deno_core,
    ..
  } = &generator_state;

  let input_types = inputs.iter().map(|fv| fv.quote_type());
  let output_type = output.quote_ctype();

  let fast_definition = quote! {
    use #deno_core::v8::fast_api::Type;
    use #deno_core::v8::fast_api::CType;
    #deno_core::v8::fast_api::FastFunction::new(
      &[ #( #input_types ),* ],
      #output_type,
      Self::#fast_function as *const ::std::ffi::c_void
    )
  };

  let output_type = output.quote_rust_type();
  let names = &inputs
    .iter()
    .enumerate()
    .map(|(i, _)| format_ident!("arg{i}"))
    .collect::<Vec<_>>();
  let types = inputs.iter().map(|rv| rv.quote_rust_type());

  let fast_fn = quote!(
    fn #fast_function(
      _: #deno_core::v8::Local<#deno_core::v8::Object>,
      #( #names: #types, )*
    ) -> #output_type {
      #(
        let #names = #names as _;
      )*
      Self::call(#(#names),*)
    }
  );

  Ok(Some((fast_definition, fast_fn)))
}
