// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use super::generator_state::GeneratorState;
use super::signature::Arg;
use super::signature::NumericArg;
use super::signature::ParsedSignature;
use super::signature::RetVal;
use super::signature::Special;
use super::V8MappingError;
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_dispatch_slow(
  generator_state: &mut GeneratorState,
  signature: &ParsedSignature,
) -> Result<TokenStream, V8MappingError> {
  let mut output = TokenStream::new();
  for (index, arg) in signature.args.iter().enumerate() {
    output.extend(extract_arg(generator_state, index)?);
    output.extend(from_arg(generator_state, index, arg)?);
  }
  output.extend(call(generator_state));
  output.extend(return_value(generator_state, &signature.ret_val));

  let GeneratorState {
    deno_core,
    scope,
    fn_args,
    retval,
    info,
    slow_function,
    ..
  } = &generator_state;

  let with_scope = if generator_state.needs_scope {
    quote!(let #scope = &mut unsafe { #deno_core::v8::CallbackScope::new(&*#info) };)
  } else {
    quote!()
  };

  let with_retval = if generator_state.needs_retval {
    quote!(let mut #retval = #deno_core::v8::ReturnValue::from_function_callback_info(unsafe { &*#info });)
  } else {
    quote!()
  };

  let with_args = if generator_state.needs_args {
    quote!(let #fn_args = #deno_core::v8::FunctionCallbackArguments::from_function_callback_info(unsafe { &*#info });)
  } else {
    quote!()
  };

  Ok(quote! {
    pub extern "C" fn #slow_function(#info: *const #deno_core::v8::FunctionCallbackInfo) {
    #with_scope
    #with_retval
    #with_args

    #output
  }})
}

pub fn extract_arg(
  generator_state: &mut GeneratorState,
  index: usize,
) -> Result<TokenStream, V8MappingError> {
  let GeneratorState { fn_args, .. } = &generator_state;
  let arg_ident = generator_state.args.get(index);

  Ok(quote!(
    let #arg_ident = #fn_args.get(#index as i32);
  ))
}

pub fn from_arg(
  mut generator_state: &mut GeneratorState,
  index: usize,
  arg: &Arg,
) -> Result<TokenStream, V8MappingError> {
  let GeneratorState {
    deno_core, args, ..
  } = &mut generator_state;
  let arg_ident = args.get_mut(index).expect("Argument at index was missing");

  let res = match arg {
    Arg::Numeric(NumericArg::bool) => quote! {
      let #arg_ident = #arg_ident.is_true();
    },
    Arg::Numeric(NumericArg::u8)
    | Arg::Numeric(NumericArg::u16)
    | Arg::Numeric(NumericArg::u32) => {
      quote! {
        let #arg_ident = #deno_core::_ops::to_u32(&#arg_ident) as _;
      }
    }
    Arg::Numeric(NumericArg::i8)
    | Arg::Numeric(NumericArg::i16)
    | Arg::Numeric(NumericArg::i32)
    | Arg::Numeric(NumericArg::__SMI__) => {
      quote! {
        let #arg_ident = #deno_core::_ops::to_i32(&#arg_ident) as _;
      }
    }
    Arg::Numeric(NumericArg::u64) | Arg::Numeric(NumericArg::usize) => {
      quote! {
        let #arg_ident = #deno_core::_ops::to_u64(&#arg_ident) as _;
      }
    }
    Arg::Numeric(NumericArg::i64) | Arg::Numeric(NumericArg::isize) => {
      quote! {
        let #arg_ident = #deno_core::_ops::to_i64(&#arg_ident) as _;
      }
    }
    Arg::OptionNumeric(numeric) => {
      // Ends the borrow of generator_state
      let arg_ident = arg_ident.clone();
      let some = from_arg(generator_state, index, &Arg::Numeric(*numeric))?;
      quote! {
        let #arg_ident = if #arg_ident.is_null_or_undefined() {
          None
        } else {
          #some
          Some(#arg_ident)
        };
      }
    }
    Arg::Option(Special::String) => {
      quote! {
        let #arg_ident = #arg_ident.to_rust_string_lossy();
      }
    }
    Arg::Special(Special::RefStr) => {
      quote! {
        let #arg_ident = #arg_ident.to_rust_string_lossy();
      }
    }
    _ => return Err(V8MappingError::NoMapping("a slow argument", arg.clone())),
  };
  Ok(res)
}

pub fn call(
  generator_state: &mut GeneratorState,
) -> Result<TokenStream, V8MappingError> {
  let GeneratorState { result, .. } = &generator_state;

  let mut tokens = TokenStream::new();
  for arg in &generator_state.args {
    tokens.extend(quote!( #arg , ));
  }
  Ok(quote! {
    let #result = Self::call( #tokens );
  })
}

pub fn return_value(
  generator_state: &mut GeneratorState,
  ret_type: &RetVal,
) -> Result<TokenStream, V8MappingError> {
  match ret_type {
    RetVal::Infallible(ret_type) => {
      return_value_infallible(generator_state, ret_type)
    }
    RetVal::Result(ret_type) => return_value_result(generator_state, ret_type),
  }
}

pub fn return_value_infallible(
  generator_state: &mut GeneratorState,
  ret_type: &Arg,
) -> Result<TokenStream, V8MappingError> {
  let GeneratorState {
    result,
    retval,
    needs_retval,
    ..
  } = generator_state;

  let res = match ret_type {
    Arg::Numeric(NumericArg::u8)
    | Arg::Numeric(NumericArg::u16)
    | Arg::Numeric(NumericArg::u32) => {
      *needs_retval = true;
      quote!(#retval.set_uint32(#result as u32);)
    }
    Arg::Numeric(NumericArg::i8)
    | Arg::Numeric(NumericArg::i16)
    | Arg::Numeric(NumericArg::i32) => {
      *needs_retval = true;
      quote!(#retval.set_int32(#result as i32);)
    }
    _ => {
      return Err(V8MappingError::NoMapping(
        "a slow return value",
        ret_type.clone(),
      ))
    }
  };

  Ok(res)
}

pub fn return_value_result(
  generator_state: &mut GeneratorState,
  ret_type: &Arg,
) -> Result<TokenStream, V8MappingError> {
  let infallible = return_value_infallible(generator_state, ret_type)?;
  let GeneratorState { result, .. } = &generator_state;

  let tokens = quote!(
    let result = match ret_type {
      Ok(#result) => {
        #infallible,
      }
      Err(err) => {
        return;
      }
    }
  );
  Ok(tokens)
}
