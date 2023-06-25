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
pub(crate) enum V8FastCallType {
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
  CallbackOptions,
}

impl V8FastCallType {
  /// Quote fast value type.
  fn quote_rust_type(&self, deno_core: &TokenStream) -> TokenStream {
    match self {
      V8FastCallType::Void => quote!(()),
      V8FastCallType::Bool => quote!(bool),
      V8FastCallType::U32 => quote!(u32),
      V8FastCallType::I32 => quote!(i32),
      V8FastCallType::U64 => quote!(u64),
      V8FastCallType::I64 => quote!(i64),
      V8FastCallType::F32 => quote!(f32),
      V8FastCallType::F64 => quote!(f64),
      V8FastCallType::Pointer => quote!(*mut ::std::ffi::c_void),
      V8FastCallType::V8Value => {
        quote!(#deno_core::v8::Local<#deno_core::v8::Value>)
      }
      V8FastCallType::CallbackOptions => {
        quote!(*mut #deno_core::v8::fast_api::FastApiCallbackOptions)
      }
      V8FastCallType::Uint8Array
      | V8FastCallType::Uint32Array
      | V8FastCallType::Float64Array
      | V8FastCallType::SeqOneByteString => unreachable!(),
    }
  }

  /// Quote fast value type's variant.
  fn quote_ctype(&self) -> TokenStream {
    match &self {
      V8FastCallType::Void => quote!(CType::Void),
      V8FastCallType::Bool => quote!(CType::Bool),
      V8FastCallType::U32 => quote!(CType::Uint32),
      V8FastCallType::I32 => quote!(CType::Int32),
      V8FastCallType::U64 => quote!(CType::Uint64),
      V8FastCallType::I64 => quote!(CType::Int64),
      V8FastCallType::F32 => quote!(CType::Float32),
      V8FastCallType::F64 => quote!(CType::Float64),
      V8FastCallType::Pointer => quote!(CType::Pointer),
      V8FastCallType::V8Value => quote!(CType::V8Value),
      V8FastCallType::CallbackOptions => quote!(CType::CallbackOptions),
      V8FastCallType::Uint8Array => unreachable!(),
      V8FastCallType::Uint32Array => unreachable!(),
      V8FastCallType::Float64Array => unreachable!(),
      V8FastCallType::SeqOneByteString => quote!(CType::SeqOneByteString),
    }
  }

  /// Quote fast value type's variant.
  fn quote_type(&self) -> TokenStream {
    match &self {
      V8FastCallType::Void => quote!(Type::Void),
      V8FastCallType::Bool => quote!(Type::Bool),
      V8FastCallType::U32 => quote!(Type::Uint32),
      V8FastCallType::I32 => quote!(Type::Int32),
      V8FastCallType::U64 => quote!(Type::Uint64),
      V8FastCallType::I64 => quote!(Type::Int64),
      V8FastCallType::F32 => quote!(Type::Float32),
      V8FastCallType::F64 => quote!(Type::Float64),
      V8FastCallType::Pointer => quote!(Type::Pointer),
      V8FastCallType::V8Value => quote!(Type::V8Value),
      V8FastCallType::CallbackOptions => quote!(Type::CallbackOptions),
      V8FastCallType::Uint8Array => quote!(Type::TypedArray(CType::Uint8)),
      V8FastCallType::Uint32Array => quote!(Type::TypedArray(CType::Uint32)),
      V8FastCallType::Float64Array => quote!(Type::TypedArray(CType::Float64)),
      V8FastCallType::SeqOneByteString => quote!(Type::SeqOneByteString),
    }
  }
}

pub fn generate_dispatch_fast(
  generator_state: &mut GeneratorState,
  signature: &ParsedSignature,
) -> Result<Option<(TokenStream, TokenStream)>, V8MappingError> {
  let mut inputs = vec![];
  for arg in &signature.args {
    let Some(fv) = map_arg_to_v8_fastcall_type(arg)? else {
      return Ok(None);
    };
    inputs.push(fv);
  }
  let mut names = inputs
    .iter()
    .enumerate()
    .map(|(i, _)| format_ident!("arg{i}"))
    .collect::<Vec<_>>();

  let ret_val = match &signature.ret_val {
    RetVal::Infallible(arg) => arg,
    RetVal::Result(arg) => arg,
  };

  let output = match map_retval_to_v8_fastcall_type(ret_val)? {
    None => return Ok(None),
    Some(rv) => rv,
  };

  let GeneratorState {
    fast_function,
    deno_core,
    result,
    opctx,
    fast_api_callback_options,
    needs_fast_api_callback_options,
    needs_fast_opctx,
    ..
  } = generator_state;

  let handle_error = match signature.ret_val {
    RetVal::Infallible(_) => quote!(),
    RetVal::Result(_) => {
      *needs_fast_api_callback_options = true;
      *needs_fast_opctx = true;
      inputs.push(V8FastCallType::CallbackOptions);
      quote! {
        let #result = match #result {
          Ok(#result) => #result,
          Err(err) => {
            // FASTCALL FALLBACK: This is where we set the errors for the slow-call error pickup path. There
            // is no code running between this and the other FASTCALL FALLBACK comment, except some V8 code
            // required to perform the fallback process. This is why the below call is safe.

            // The reason we need to do this is because V8 does not allow exceptions to be thrown from the
            // fast call. Instead, you are required to set the fallback flag, which indicates to V8 that it
            // should re-call the slow version of the function. Technically the slow call should perform the
            // same operation and then throw the same error (because it should be idempotent), but in our
            // case we stash the error and pick it up on the slow path before doing any work.

            // TODO(mmastrac): We should allow an #[op] flag to re-perform slow calls without the error path when
            // the method is performance sensitive.

            // SAFETY: We guarantee that OpCtx has no mutable references once ops are live and being called,
            // allowing us to perform this one little bit of mutable magic.
            unsafe { #opctx.unsafely_set_last_error_for_ops_only(err); }
            #fast_api_callback_options.fallback = true;
            return ::std::default::Default::default();
          }
        };
      }
    }
  };

  let input_types = inputs.iter().map(|fv| fv.quote_type()).collect::<Vec<_>>();
  let output_type = output.quote_ctype();

  let fast_definition = quote! {
    use #deno_core::v8::fast_api::Type;
    use #deno_core::v8::fast_api::CType;
    #deno_core::v8::fast_api::FastFunction::new(
      &[ Type::V8Value, #( #input_types ),* ],
      #output_type,
      Self::#fast_function as *const ::std::ffi::c_void
    )
  };

  let output_type = output.quote_rust_type(deno_core);
  let mut types = inputs
    .iter()
    .map(|rv| rv.quote_rust_type(deno_core))
    .collect::<Vec<_>>();

  let call_args = names.clone();

  let with_fast_api_callback_options = if *needs_fast_api_callback_options {
    types.push(V8FastCallType::CallbackOptions.quote_rust_type(deno_core));
    names.push(fast_api_callback_options.clone());
    quote! {
      let #fast_api_callback_options = unsafe { &mut *#fast_api_callback_options };
    }
  } else {
    quote!()
  };
  let with_opctx = if *needs_fast_opctx {
    quote!(
      let #opctx = unsafe {
        &*(#deno_core::v8::Local::<v8::External>::cast(unsafe { #fast_api_callback_options.data.data }).value()
            as *const #deno_core::_ops::OpCtx)
      };
    )
  } else {
    quote!()
  };

  let fast_fn = quote!(
    fn #fast_function(
      _: #deno_core::v8::Local<#deno_core::v8::Object>,
      #( #names: #types, )*
    ) -> #output_type {
      #with_fast_api_callback_options
      #with_opctx
      let #result = Self::call(#(#call_args as _),*);
      #handle_error
      #result
    }
  );

  Ok(Some((fast_definition, fast_fn)))
}

fn map_arg_to_v8_fastcall_type(
  arg: &Arg,
) -> Result<Option<V8FastCallType>, V8MappingError> {
  let rv = match arg {
    Arg::OptionNumeric(_) | Arg::SerdeV8(_) => return Ok(None),
    Arg::Numeric(NumericArg::bool) => V8FastCallType::Bool,
    Arg::Numeric(NumericArg::u32)
    | Arg::Numeric(NumericArg::u16)
    | Arg::Numeric(NumericArg::u8) => V8FastCallType::U32,
    Arg::Numeric(NumericArg::i32)
    | Arg::Numeric(NumericArg::i16)
    | Arg::Numeric(NumericArg::i8)
    | Arg::Numeric(NumericArg::__SMI__) => V8FastCallType::I32,
    Arg::Numeric(NumericArg::u64) | Arg::Numeric(NumericArg::usize) => {
      V8FastCallType::U64
    }
    Arg::Numeric(NumericArg::i64) | Arg::Numeric(NumericArg::isize) => {
      V8FastCallType::I64
    }
    _ => return Err(V8MappingError::NoMapping("a fast argument", arg.clone())),
  };
  Ok(Some(rv))
}

fn map_retval_to_v8_fastcall_type(
  arg: &Arg,
) -> Result<Option<V8FastCallType>, V8MappingError> {
  let rv = match arg {
    Arg::OptionNumeric(_) | Arg::SerdeV8(_) => return Ok(None),
    Arg::Void => V8FastCallType::Void,
    Arg::Numeric(NumericArg::bool) => V8FastCallType::Bool,
    Arg::Numeric(NumericArg::u32)
    | Arg::Numeric(NumericArg::u16)
    | Arg::Numeric(NumericArg::u8) => V8FastCallType::U32,
    Arg::Numeric(NumericArg::i32)
    | Arg::Numeric(NumericArg::i16)
    | Arg::Numeric(NumericArg::i8) => V8FastCallType::I32,
    Arg::Numeric(NumericArg::u64) | Arg::Numeric(NumericArg::usize) => {
      V8FastCallType::U64
    }
    Arg::Numeric(NumericArg::i64) | Arg::Numeric(NumericArg::isize) => {
      V8FastCallType::I64
    }
    Arg::Special(_) => return Ok(None),
    _ => {
      return Err(V8MappingError::NoMapping(
        "a fast return value",
        arg.clone(),
      ))
    }
  };
  Ok(Some(rv))
}
