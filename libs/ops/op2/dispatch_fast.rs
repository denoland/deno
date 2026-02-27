// Copyright 2018-2025 the Deno authors. MIT license.

use super::V8MappingError;
use super::V8SignatureMappingError;
use super::config::MacroConfig;
use super::dispatch_shared::byte_slice_to_buffer;
use super::dispatch_shared::v8_intermediate_to_arg;
use super::dispatch_shared::v8_to_arg;
use super::dispatch_shared::v8slice_to_buffer;
use super::generator_state::GeneratorState;
use super::generator_state::gs_quote;
use super::signature::Arg;
use super::signature::BufferMode;
use super::signature::BufferSource;
use super::signature::BufferType;
use super::signature::NumericArg;
use super::signature::NumericFlag;
use super::signature::ParsedSignature;
use super::signature::RefType;
use super::signature::Special;
use super::signature::Strings;
use crate::op2::dispatch_async::map_async_return_type;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::Type;

#[derive(Clone)]
pub(crate) enum FastArg {
  /// The argument is virtual and only has an output name.
  Virtual {
    name_out: Ident,
    arg: Arg,
  },
  Actual {
    arg_type: V8FastCallType,
    name_in: Ident,
    name_out: Ident,
    arg: Arg,
  },
  CallbackOptions,
  PromiseId,
}

#[derive(Clone)]
pub(crate) struct FastSignature {
  // The parsed arguments
  pub args: Vec<FastArg>,
  // The parsed return value
  pub ret_val: V8FastCallType,
  has_fast_api_callback_options: bool,
}

impl FastSignature {
  /// Collect the output of `quote_type` for all actual arguments, used to populate the fast function
  /// definition struct.
  pub(crate) fn input_types(&self) -> Vec<TokenStream> {
    self
      .args
      .iter()
      .filter_map(|arg| match arg {
        FastArg::PromiseId => Some(V8FastCallType::I32.quote_ctype()),
        FastArg::CallbackOptions => {
          Some(V8FastCallType::CallbackOptions.quote_ctype())
        }
        FastArg::Actual { arg_type, .. } => Some(arg_type.quote_ctype()),
        _ => None,
      })
      .collect()
  }

  pub(crate) fn input_args(
    &self,
    generator_state: &GeneratorState,
  ) -> Vec<(Ident, TokenStream)> {
    self
      .args
      .iter()
      .filter_map(|arg| match arg {
        FastArg::CallbackOptions => Some((
          generator_state.fast_api_callback_options.clone(),
          V8FastCallType::CallbackOptions.quote_rust_type(),
        )),
        FastArg::PromiseId => Some((
          generator_state.promise_id.clone(),
          V8FastCallType::I32.quote_rust_type(),
        )),
        FastArg::Actual {
          arg_type, name_in, ..
        } => Some((format_ident!("{name_in}"), arg_type.quote_rust_type())),
        _ => None,
      })
      .collect()
  }

  pub(crate) fn call_args(
    &self,
    generator_state: &mut GeneratorState,
    arg_spans: &[Span],
  ) -> Result<Vec<TokenStream>, V8SignatureMappingError> {
    let mut call_args = vec![];
    for (idx, arg) in self.args.iter().enumerate() {
      match arg {
        FastArg::Actual { arg, name_out, .. }
        | FastArg::Virtual { name_out, arg } => {
          if !matches!(arg, Arg::Ref(_, Special::HandleScope)) {
            let span =
              arg_spans.get(idx).copied().unwrap_or_else(Span::call_site);
            call_args.push(
              map_v8_fastcall_arg_to_arg(generator_state, name_out, arg)
                .map_err(|s| {
                  V8SignatureMappingError::NoArgMapping(
                    span,
                    s,
                    Box::new(arg.clone()),
                  )
                })?,
            )
          } else {
            generator_state.needs_scope = true;
          }
        }
        FastArg::CallbackOptions | FastArg::PromiseId => {}
      }
    }
    Ok(call_args)
  }

  pub(crate) fn call_names(&self) -> Vec<TokenStream> {
    let mut call_names = vec![];
    for arg in &self.args {
      match arg {
        FastArg::Actual { name_out, arg, .. }
        | FastArg::Virtual { name_out, arg } => {
          if matches!(arg, Arg::Ref(_, Special::HandleScope)) {
            call_names.push(quote!(&mut scope));
          } else {
            call_names.push(quote!(#name_out));
          }
        }
        FastArg::CallbackOptions | FastArg::PromiseId => {}
      }
    }
    call_names
  }

  pub(crate) fn get_fast_function_def(
    &self,
    fast_function: &Ident,
  ) -> TokenStream {
    let input_types = self.input_types();
    let output_type = self.ret_val.quote_ctype();

    quote!(
      use deno_core::v8::fast_api::Type as CType;
      use deno_core::v8;

      deno_core::v8::fast_api::CFunction::new(
        Self::#fast_function as _,
        &deno_core::v8::fast_api::CFunctionInfo::new(
          #output_type,
          &[ CType::V8Value.as_info(), #( #input_types ),* ],
          deno_core::v8::fast_api::Int64Representation::BigInt,
        ),
      )
    )
  }

  pub(crate) fn ensure_fast_api_callback_options(&mut self) {
    if !self.has_fast_api_callback_options {
      self.has_fast_api_callback_options = true;
      self.args.push(FastArg::CallbackOptions);
    }
  }

  fn insert_promise_id(&mut self) {
    self.args.insert(0, FastArg::PromiseId)
  }
}

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
  /// Any typed array.
  AnyArray,
  Uint8Array,
  Uint32Array,
  Float32Array,
  Float64Array,
  SeqOneByteString,
  CallbackOptions,
  /// ArrayBuffers are currently supported in fastcalls by passing a V8Value and manually unwrapping
  /// the buffer. In the future, V8 may be able to support ArrayBuffer fastcalls in the same way that
  /// a TypedArray overload works and we may be able to adjust the support here.
  ArrayBuffer,
  /// Used for virtual arguments that do not contribute a raw argument
  Virtual,
}

impl V8FastCallType {
  /// Quote fast value type.
  fn quote_rust_type(&self) -> TokenStream {
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
      V8FastCallType::V8Value
      | V8FastCallType::Uint8Array
      | V8FastCallType::Uint32Array
      | V8FastCallType::Float32Array
      | V8FastCallType::Float64Array => {
        quote!(deno_core::v8::Local<deno_core::v8::Value>)
      }
      V8FastCallType::CallbackOptions => {
        quote!(*mut deno_core::v8::fast_api::FastApiCallbackOptions<'s>)
      }
      V8FastCallType::SeqOneByteString => {
        quote!(*mut deno_core::v8::fast_api::FastApiOneByteString)
      }
      V8FastCallType::AnyArray | V8FastCallType::ArrayBuffer => {
        quote!(deno_core::v8::Local<deno_core::v8::Value>)
      }
      V8FastCallType::Virtual => unreachable!("invalid virtual argument"),
    }
  }

  /// Quote fast value type's variant.
  fn quote_ctype(&self) -> TokenStream {
    match &self {
      V8FastCallType::Void => quote!(CType::Void.as_info()),
      V8FastCallType::Bool => quote!(CType::Bool.as_info()),
      V8FastCallType::U32 => quote!(v8::fast_api::CTypeInfo::new(
        CType::Uint32,
        v8::fast_api::Flags::empty(),
      )),
      V8FastCallType::I32 => quote!(v8::fast_api::CTypeInfo::new(
        CType::Int32,
        v8::fast_api::Flags::empty(),
      )),
      V8FastCallType::U64 => quote!(CType::Uint64.as_info()),
      V8FastCallType::I64 => quote!(CType::Int64.as_info()),
      V8FastCallType::F32 => quote!(CType::Float32.as_info()),
      V8FastCallType::F64 => quote!(CType::Float64.as_info()),
      V8FastCallType::Pointer => quote!(CType::Pointer.as_info()),
      V8FastCallType::V8Value => quote!(CType::V8Value.as_info()),
      V8FastCallType::CallbackOptions => {
        quote!(CType::CallbackOptions.as_info())
      }
      V8FastCallType::AnyArray => quote!(CType::V8Value.as_info()),
      V8FastCallType::Uint8Array => quote!(CType::V8Value.as_info()),
      V8FastCallType::Uint32Array => quote!(CType::V8Value.as_info()),
      V8FastCallType::Float32Array => quote!(CType::V8Value.as_info()),
      V8FastCallType::Float64Array => quote!(CType::V8Value.as_info()),
      V8FastCallType::SeqOneByteString => {
        quote!(CType::SeqOneByteString.as_info())
      }
      V8FastCallType::ArrayBuffer => quote!(CType::V8Value.as_info()),
      V8FastCallType::Virtual => unreachable!("invalid virtual argument"),
    }
  }
}

// TODO(mmastrac): see note about index_in below
#[allow(clippy::explicit_counter_loop)]
pub(crate) fn get_fast_signature(
  signature: &ParsedSignature,
) -> Result<Option<FastSignature>, V8SignatureMappingError> {
  let mut args = vec![];
  let mut index_in = 0;
  for (index_out, (arg, _)) in signature.args.iter().cloned().enumerate() {
    let arg_span = signature
      .arg_spans
      .get(index_out)
      .copied()
      .unwrap_or_else(Span::call_site);
    let Some(arg_type) = map_arg_to_v8_fastcall_type(&arg).map_err(|s| {
      V8SignatureMappingError::NoArgMapping(arg_span, s, Box::new(arg.clone()))
    })?
    else {
      return Ok(None);
    };
    let name_out = format_ident!("arg{index_out}");
    // TODO(mmastrac): this could be a valid arg, but we need to update has_fast_api_callback_options below
    assert!(arg_type != V8FastCallType::CallbackOptions);
    if arg_type == V8FastCallType::Virtual {
      args.push(FastArg::Virtual { arg, name_out });
    } else {
      args.push(FastArg::Actual {
        arg,
        arg_type,
        name_in: format_ident!("arg{index_in}"),
        name_out,
      });
    }
    // TODO(mmastrac): these fastcall indexes should not use the same index as the outparam
    index_in += 1;
  }

  let ret_val = if signature.ret_val.is_async() {
    &Arg::Void
  } else {
    signature.ret_val.arg()
  };
  let output = match map_retval_to_v8_fastcall_type(ret_val).map_err(|s| {
    V8SignatureMappingError::NoRetValMapping(
      signature.ret_span,
      s,
      Box::new(signature.ret_val.clone()),
    )
  })? {
    None => return Ok(None),
    Some(rv) => rv,
  };

  Ok(Some(FastSignature {
    args,
    ret_val: output,
    has_fast_api_callback_options: false,
  }))
}

fn create_scope(generator_state: &mut GeneratorState) -> TokenStream {
  generator_state.needs_fast_api_callback_options = true;
  gs_quote!(generator_state(fast_api_callback_options) => {
    // SAFETY: This is using an &FastApiCallbackOptions inside a fast call.
    unsafe {
      deno_core::v8::CallbackScope::new(&*#fast_api_callback_options)
    }
  })
}

fn throw_type_error(
  generator_state: &mut GeneratorState,
  message: impl std::fmt::Display,
) -> TokenStream {
  let create_scope = create_scope(generator_state);
  let message = format!("{message}");
  quote!({
    let scope = ::std::pin::pin!(#create_scope);
    let mut scope = scope.init();
    deno_core::_ops::throw_error_one_byte(&mut scope, #message);
    // SAFETY: All fast return types have zero as a valid value
    return unsafe { std::mem::zeroed() };
  })
}

/// Sheds the error in a `Result<T, E>` as an early return, leaving just the `T` and requesting
/// that v8 re-call the slow function to throw the error.
pub(crate) fn generate_fast_result_early_exit(
  generator_state: &mut GeneratorState,
) -> TokenStream {
  generator_state.needs_opctx = true;
  let create_scope = create_scope(generator_state);
  gs_quote!(generator_state(result) => {
    let #result = match #result {
      Ok(#result) => #result,
      Err(err) => {
        let scope = ::std::pin::pin!(#create_scope);
        let mut scope = scope.init();
        let exception = deno_core::error::to_v8_error(
          &mut scope,
          &err,
        );
        scope.throw_exception(exception);
        // SAFETY: All fast return types have zero as a valid value
        return unsafe { std::mem::zeroed() };
      }
    };
  })
}

pub(crate) fn generate_dispatch_fast(
  config: &MacroConfig,
  generator_state: &mut GeneratorState,
  signature: &ParsedSignature,
) -> Result<
  Option<(TokenStream, TokenStream, TokenStream)>,
  V8SignatureMappingError,
> {
  if let Some(alternative) = &config.fast_alternative {
    // TODO(mmastrac): we should validate the alternatives. For now we just assume the caller knows what
    // they are doing.
    let alternative = syn::parse_str::<Type>(alternative).map_err(|_| {
      V8SignatureMappingError::NoRetValMapping(
        Span::call_site(),
        "failed to reparse fast alternative type",
        Box::new(signature.ret_val.clone()),
      )
    })?;
    return Ok(Some((
      quote!(#alternative().fast_fn()),
      quote!(#alternative().fast_fn_with_metrics()),
      quote!(),
    )));
  }

  // async(lazy) can be fast
  if signature.ret_val.is_async()
    && !config.async_lazy
    && !config.async_deferred
    || config.fake_async
  {
    return Ok(None);
  }

  let Some(mut fastsig) = get_fast_signature(signature)? else {
    return Ok(None);
  };

  // TODO(mmastrac): we should save this unwrapped result
  let handle_error = match signature.ret_val.unwrap_result() {
    Some(_) => generate_fast_result_early_exit(generator_state),
    _ => quote!(),
  };

  if signature.ret_val.is_async() {
    fastsig.insert_promise_id();
  }

  // Note that this triggers needs_* values in generator_state
  let call_args = fastsig.call_args(generator_state, &signature.arg_spans)?;

  let handle_result = if signature.ret_val.is_async() {
    generator_state.needs_opctx = true;
    let (return_value, mapper, _) =
      map_async_return_type(generator_state, &signature.ret_val).map_err(
        |s| {
          V8SignatureMappingError::NoRetValMapping(
            signature.ret_span,
            s,
            Box::new(signature.ret_val.clone()),
          )
        },
      )?;

    let lazy = config.async_lazy;
    let deferred = config.async_deferred;
    gs_quote!(generator_state(promise_id, result, opctx, scope) => {
      // Lazy results will always return None
      deno_core::_ops::#mapper(#opctx, #lazy, #deferred, #promise_id, #result, |#scope, #result| {
        #return_value
      });
    })
  } else {
    gs_quote!(generator_state(result) => {
      // Result may need a simple cast (eg: SMI u32->i32)
      #result as _
    })
  };

  let with_stack_trace = if generator_state.needs_stack_trace {
    generator_state.needs_opctx = true;
    generator_state.needs_scope = true;

    gs_quote!(generator_state(opctx, scope, opstate) =>
    (if #opctx.enable_stack_trace {
      let stack_trace_msg = deno_core::v8::String::empty(&mut #scope);
      let stack_trace_error = deno_core::v8::Exception::error(&mut #scope, stack_trace_msg.into());
      let js_error = deno_core::error::JsError::from_v8_exception(&mut #scope, stack_trace_error);
      let mut op_state = ::std::cell::RefCell::borrow_mut(&#opstate);
      op_state.op_stack_trace_callback.as_ref().unwrap()(js_error.frames)
    })
    )
  } else {
    quote!()
  };

  let with_opstate =
    if generator_state.needs_opstate || generator_state.needs_stack_trace {
      generator_state.needs_opctx = true;
      gs_quote!(generator_state(opctx, opstate) =>
        (let #opstate = &#opctx.state;)
      )
    } else {
      quote!()
    };

  let with_js_runtime_state = if generator_state.needs_js_runtime_state {
    generator_state.needs_opctx = true;
    gs_quote!(generator_state(js_runtime_state, opctx) => {
      let #js_runtime_state = #opctx.runtime_state();
    })
  } else {
    quote!()
  };

  let with_opctx = if generator_state.needs_opctx {
    generator_state.needs_fast_api_callback_options = true;
    gs_quote!(generator_state(opctx, fast_api_callback_options) => {
      let #opctx: &'s _ = unsafe {
        &*(deno_core::v8::Local::<deno_core::v8::External>::cast_unchecked(unsafe { #fast_api_callback_options.data }).value()
            as *const deno_core::_ops::OpCtx)
      };
    })
  } else {
    quote!()
  };

  let with_self = if generator_state.needs_self {
    generator_state.needs_fast_isolate = true;
    let throw_exception = throw_type_error(
      generator_state,
      format!("expected {}", &generator_state.self_ty),
    );
    gs_quote!(generator_state(self_ty, scope, try_unwrap_cppgc) => {
      let Some(self_) = deno_core::_ops::#try_unwrap_cppgc::<#self_ty>(&mut #scope, this.into()) else {
        #throw_exception
      };
      let self_ = unsafe { self_.as_ref() };
    })
  } else {
    quote!()
  };

  let with_isolate = if generator_state.needs_fast_isolate
    && !generator_state.needs_scope
    && !generator_state.needs_stack_trace
  {
    generator_state.needs_fast_api_callback_options = true;
    gs_quote!(generator_state(scope, fast_api_callback_options) =>
      (let mut #scope = unsafe { #fast_api_callback_options.isolate_unchecked_mut() };)
    )
  } else {
    quote!()
  };
  let with_scope = if generator_state.needs_scope {
    let create_scope = create_scope(generator_state);
    gs_quote!(generator_state(scope) => {
      let #scope = ::std::pin::pin!(#create_scope);
      let mut #scope = scope.init();
    })
  } else {
    quote!()
  };

  let name = &generator_state.name;
  let call = if generator_state.needs_self {
    quote!(self_. #name)
  } else {
    quote!(Self:: #name)
  };

  let mut fastsig_metrics = fastsig.clone();
  fastsig_metrics.ensure_fast_api_callback_options();

  let with_fast_api_callback_options = if generator_state
    .needs_fast_api_callback_options
  {
    fastsig.ensure_fast_api_callback_options();
    gs_quote!(generator_state(fast_api_callback_options) => {
      let #fast_api_callback_options: &'s mut _ = unsafe { &mut *#fast_api_callback_options };
    })
  } else {
    quote!()
  };

  let fast_function = generator_state.fast_function.clone();
  let fast_definition = fastsig.get_fast_function_def(&fast_function);
  let fast_function = generator_state.fast_function_metrics.clone();
  let fast_definition_metrics =
    fastsig_metrics.get_fast_function_def(&fast_function);

  let output_type = fastsig.ret_val.quote_rust_type();

  // We don't want clippy to trigger warnings on number of arguments of the fastcall
  // function -- these will still trigger on our normal call function, however.
  let call_names = fastsig.call_names();
  let (fastcall_metrics_names, fastcall_metrics_types): (Vec<_>, Vec<_>) =
    fastsig_metrics
      .input_args(generator_state)
      .into_iter()
      .unzip();
  let (fastcall_names, fastcall_types): (Vec<_>, Vec<_>) =
    fastsig.input_args(generator_state).into_iter().unzip();

  let fast_fn = gs_quote!(generator_state(result, fast_api_callback_options, fast_function, fast_function_metrics) => {
    #[allow(clippy::too_many_arguments)]
    extern "C" fn #fast_function_metrics<'s>(
      this: deno_core::v8::Local<deno_core::v8::Object>,
      #( #fastcall_metrics_names: #fastcall_metrics_types, )*
    ) -> #output_type {
      let #fast_api_callback_options: &'s mut _ =
        unsafe { &mut *#fast_api_callback_options };
      let opctx: &'s _ = unsafe {
          &*(deno_core::v8::Local::<deno_core::v8::External>::cast_unchecked(
            unsafe { #fast_api_callback_options.data }
          ).value() as *const deno_core::_ops::OpCtx)
      };
      deno_core::_ops::dispatch_metrics_fast(opctx, deno_core::_ops::OpMetricsEvent::Dispatched);
      let res = Self::#fast_function( this, #( #fastcall_names, )* );
      deno_core::_ops::dispatch_metrics_fast(opctx, deno_core::_ops::OpMetricsEvent::Completed);
      res
    }

    #[allow(clippy::too_many_arguments)]
    extern "C" fn #fast_function<'s>(
      this: deno_core::v8::Local<deno_core::v8::Object>,
      #( #fastcall_names: #fastcall_types, )*
    ) -> #output_type {
      #[cfg(debug_assertions)]
      let _reentrancy_check_guard = deno_core::_ops::reentrancy_check(&<Self as deno_core::_ops::Op>::DECL);

      #with_fast_api_callback_options
      #with_scope
      #with_opctx
      #with_opstate;
      #with_stack_trace
      #with_js_runtime_state
      #with_isolate
      #with_self
      let #result = {
        #(#call_args)*
        #call (#(#call_names),*)
      };
      #handle_error
      #handle_result
    }
  });

  Ok(Some((fast_definition, fast_definition_metrics, fast_fn)))
}

fn fast_api_typed_array_to_buffer(
  generator_state: &mut GeneratorState,
  arg_ident: &Ident,
  input: &Ident,
  buffer: BufferType,
) -> Result<TokenStream, V8MappingError> {
  let convert = byte_slice_to_buffer(arg_ident, input, buffer)?;
  let throw_exception =
    throw_type_error(generator_state, "expected ArrayBufferView");
  Ok(quote! {
    let Ok(#input) = #input.try_cast::<deno_core::v8::ArrayBufferView>() else {
        #throw_exception
    };
    let mut buffer = [0; ::deno_core::v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP];
    // SAFETY: we are certain the implied lifetime is valid here as the slices never escape the
    // fastcall.
    let #input = unsafe {
      let (input_ptr, input_len) = #input.get_contents_raw_parts(&mut buffer);
      let input_ptr = if input_ptr.is_null() { ::std::ptr::dangling_mut() } else { input_ptr };
      let slice = ::std::slice::from_raw_parts_mut::<'s>(input_ptr, input_len);
      let (before, slice, after) = slice.align_to_mut();
      debug_assert!(before.is_empty());
      debug_assert!(after.is_empty());
      slice
    };
    #convert
  })
}

#[allow(clippy::too_many_arguments)]
fn map_v8_fastcall_arg_to_arg(
  generator_state: &mut GeneratorState,
  arg_ident: &Ident,
  arg: &Arg,
) -> Result<TokenStream, V8MappingError> {
  let GeneratorState {
    opctx,
    js_runtime_state,
    scope,
    needs_opctx,
    needs_fast_api_callback_options,
    needs_fast_isolate,
    needs_js_runtime_state,
    ..
  } = generator_state;

  let arg_temp = format_ident!("{}_temp", arg_ident);

  let res = match arg {
    Arg::Buffer(
      buffer @ (BufferType::V8Slice(..) | BufferType::JsBuffer),
      _,
      BufferSource::ArrayBuffer,
    ) => {
      let throw_exception =
        throw_type_error(generator_state, "expected ArrayBuffer");
      let buf = v8slice_to_buffer(arg_ident, &arg_temp, *buffer)?;
      quote!(
        let Ok(mut #arg_temp) = deno_core::_ops::to_v8_slice_buffer(#arg_ident.into()) else {
          #throw_exception
        };
        #buf
      )
    }
    Arg::Buffer(
      buffer @ (BufferType::V8Slice(..) | BufferType::JsBuffer),
      _,
      BufferSource::TypedArray,
    ) => {
      let throw_exception =
        throw_type_error(generator_state, "expected TypedArray");
      let buf = v8slice_to_buffer(arg_ident, &arg_temp, *buffer)?;
      quote!(
        let Ok(mut #arg_temp) = deno_core::_ops::to_v8_slice(#arg_ident.into()) else {
          #throw_exception
        };
        #buf
      )
    }
    Arg::Buffer(buffer, _, BufferSource::ArrayBuffer) => {
      let throw_exception =
        throw_type_error(generator_state, "expected ArrayBuffer");
      let buf = byte_slice_to_buffer(arg_ident, &arg_temp, *buffer)?;
      quote!(
        // SAFETY: This slice doesn't outlive the function
        let Ok(mut #arg_temp) = (unsafe { deno_core::_ops::to_slice_buffer(#arg_ident.into()) }) else {
          #throw_exception
        };
        #buf
      )
    }
    Arg::Buffer(buffer, _, BufferSource::Any) => {
      let throw_exception =
        throw_type_error(generator_state, "expected any buffer");
      let buf = byte_slice_to_buffer(arg_ident, &arg_temp, *buffer)?;
      quote!(
        // SAFETY: This slice doesn't outlive the function
        let Ok(mut #arg_temp) = (unsafe { deno_core::_ops::to_slice_buffer_any(#arg_ident.into()) }) else {
          #throw_exception
        };
        #buf
      )
    }
    Arg::Buffer(buffer, _, BufferSource::TypedArray) => {
      fast_api_typed_array_to_buffer(
        generator_state,
        arg_ident,
        arg_ident,
        *buffer,
      )?
    }
    Arg::Ref(RefType::Ref, Special::Isolate) => {
      *needs_fast_api_callback_options = true;
      gs_quote!(generator_state(fast_api_callback_options) => {
        let #arg_ident = unsafe { deno_core::v8::Isolate::from_raw_isolate_ptr(#fast_api_callback_options.isolate) };
        let #arg_ident = &#arg_ident;
      })
    }
    Arg::Ref(RefType::Mut, Special::Isolate) => {
      *needs_fast_api_callback_options = true;
      gs_quote!(generator_state(fast_api_callback_options) => {
        let mut #arg_ident = unsafe { deno_core::v8::Isolate::from_raw_isolate_ptr(#fast_api_callback_options.isolate) };
        let #arg_ident = &mut #arg_ident;
      })
    }
    Arg::Ref(RefType::Ref, Special::OpState) => {
      *needs_opctx = true;
      quote!(let #arg_ident = &::std::cell::RefCell::borrow(&#opctx.state);)
    }
    Arg::Ref(RefType::Mut, Special::OpState) => {
      *needs_opctx = true;
      quote!(let #arg_ident = &mut ::std::cell::RefCell::borrow_mut(&#opctx.state);)
    }
    Arg::Ref(_, Special::HandleScope) => {
      unreachable!()
    }
    Arg::RcRefCell(Special::OpState) => {
      *needs_opctx = true;
      quote!(let #arg_ident = #opctx.state.clone();)
    }
    Arg::Ref(RefType::Ref, Special::JsRuntimeState) => {
      *needs_js_runtime_state = true;
      quote!(let #arg_ident = &#js_runtime_state;)
    }
    Arg::VarArgs => quote!(let #arg_ident = None;),
    Arg::This => {
      *needs_fast_isolate = true;
      quote!(let #arg_ident = deno_core::v8::Global::new(&mut #scope, this);)
    }
    Arg::String(Strings::RefStr) => {
      quote! {
        let mut #arg_temp: [::std::mem::MaybeUninit<u8>; deno_core::_ops::STRING_STACK_BUFFER_SIZE] = [::std::mem::MaybeUninit::uninit(); deno_core::_ops::STRING_STACK_BUFFER_SIZE];
        let #arg_ident = &deno_core::_ops::to_str_ptr(unsafe { &mut *#arg_ident }, &mut #arg_temp);
      }
    }
    Arg::String(Strings::String) => {
      quote!(let #arg_ident = deno_core::_ops::to_string_ptr(unsafe { &mut *#arg_ident });)
    }
    Arg::String(Strings::CowStr) => {
      quote! {
        let mut #arg_temp: [::std::mem::MaybeUninit<u8>; deno_core::_ops::STRING_STACK_BUFFER_SIZE] = [::std::mem::MaybeUninit::uninit(); deno_core::_ops::STRING_STACK_BUFFER_SIZE];
        let #arg_ident = deno_core::_ops::to_str_ptr(unsafe { &mut *#arg_ident }, &mut #arg_temp);
      }
    }
    Arg::String(Strings::CowByte) => {
      quote!(let #arg_ident = deno_core::_ops::to_cow_byte_ptr(unsafe { &mut *#arg_ident });)
    }
    Arg::V8Local(v8)
    | Arg::OptionV8Local(v8)
    | Arg::V8Ref(_, v8)
    | Arg::OptionV8Ref(_, v8) => {
      let arg_ident = arg_ident.clone();
      let throw_type_error = || {
        Ok(throw_type_error(
          generator_state,
          format!("expected {v8:?}"),
        ))
      };
      let extract_intermediate = v8_intermediate_to_arg(&arg_ident, arg);
      v8_to_arg(v8, &arg_ident, arg, throw_type_error, extract_intermediate)?
    }
    Arg::CppGcResource(_, ty) => {
      let ty = syn::parse_str::<syn::Path>(ty)
        .map_err(|_| "failed to reparse type")?;

      *needs_fast_isolate = true;
      let throw_exception =
        throw_type_error(generator_state, format!("expected {ty:?}"));
      gs_quote!(generator_state(scope, try_unwrap_cppgc) => {
        let Some(#arg_ident) = deno_core::_ops::#try_unwrap_cppgc::<#ty>(&mut #scope, #arg_ident) else {
          #throw_exception
        };
        let #arg_ident = unsafe { #arg_ident.as_ref() };
      })
    }
    Arg::OptionCppGcResource(ty) => {
      *needs_fast_isolate = true;
      let throw_exception =
        throw_type_error(generator_state, format!("expected {ty}"));
      let ty = syn::parse_str::<syn::Path>(ty)
        .map_err(|_| "failed to reparse type")?;
      gs_quote!(generator_state(scope, try_unwrap_cppgc) => {
        let #arg_ident = if #arg_ident.is_null_or_undefined() {
          None
        } else if let Some(#arg_ident) = deno_core::_ops::#try_unwrap_cppgc::<#ty>(&mut #scope, #arg_ident) {
          Some(#arg_ident)
        } else {
          #throw_exception
        };
        let #arg_ident = unsafe { #arg_ident.as_ref().map(|a| a.as_ref()) };
      })
    }
    _ => quote!(let #arg_ident = #arg_ident as _;),
  };
  Ok(res)
}

fn map_arg_to_v8_fastcall_type(
  arg: &Arg,
) -> Result<Option<V8FastCallType>, V8MappingError> {
  let rv = match arg {
    // We don't allow detaching buffers in fast mode
    Arg::Buffer(_, BufferMode::Detach, _) => return Ok(None),
    // We don't allow JsBuffer or V8Slice fastcalls for TypedArray
    // TODO(mmastrac): we can enable these soon
    Arg::Buffer(
      BufferType::JsBuffer | BufferType::V8Slice(..),
      _,
      BufferSource::TypedArray,
      // This cannot be fast at this time as we have no way of accessing
      // the shared pointer to the backing store in a fastcall.
      // https://github.com/denoland/deno_core/issues/417
      // ) => V8FastCallType::V8Value,
    ) => return Ok(None),
    Arg::Buffer(
      BufferType::Slice(.., NumericArg::u8)
      | BufferType::Ptr(.., NumericArg::u8),
      _,
      BufferSource::Any,
      // This cannot be fast at this time as we have no way of accessing
      // the shared pointer to the backing store in a fastcall.
      // https://github.com/denoland/deno_core/issues/417
      // ) => V8FastCallType::AnyArray,
    ) => return Ok(None),
    // TODO(mmastrac): implement fast for any Any-typed buffer
    Arg::Buffer(_, _, BufferSource::Any) => return Ok(None),
    Arg::Buffer(_, _, BufferSource::ArrayBuffer) => V8FastCallType::ArrayBuffer,
    Arg::Buffer(
      BufferType::Slice(.., NumericArg::u32)
      | BufferType::Ptr(.., NumericArg::u32)
      | BufferType::Vec(.., NumericArg::u32)
      | BufferType::BoxSlice(.., NumericArg::u32),
      _,
      BufferSource::TypedArray,
    ) => V8FastCallType::Uint32Array,
    Arg::Buffer(
      BufferType::Slice(.., NumericArg::f32)
      | BufferType::Ptr(.., NumericArg::f32)
      | BufferType::Vec(.., NumericArg::f32)
      | BufferType::BoxSlice(.., NumericArg::f32),
      _,
      BufferSource::TypedArray,
    ) => V8FastCallType::Float32Array,
    Arg::Buffer(
      BufferType::Slice(.., NumericArg::f64)
      | BufferType::Ptr(.., NumericArg::f64)
      | BufferType::Vec(.., NumericArg::f64)
      | BufferType::BoxSlice(.., NumericArg::f64),
      _,
      BufferSource::TypedArray,
    ) => V8FastCallType::Float64Array,
    Arg::Buffer(_, _, BufferSource::TypedArray) => V8FastCallType::Uint8Array,
    // Virtual OpState arguments
    Arg::RcRefCell(Special::OpState)
    | Arg::Ref(_, Special::OpState)
    | Arg::Ref(RefType::Mut, Special::HandleScope)
    | Arg::Rc(Special::JsRuntimeState)
    | Arg::Ref(RefType::Ref, Special::JsRuntimeState)
    | Arg::VarArgs
    | Arg::This
    | Arg::Ref(_, Special::Isolate) => V8FastCallType::Virtual,
    // Other types + ref types are not handled
    Arg::OptionNumeric(..)
    | Arg::Option(_)
    | Arg::OptionString(_)
    | Arg::OptionBuffer(..)
    | Arg::SerdeV8(_)
    | Arg::FromV8(_, _)
    | Arg::WebIDL(_, _, _)
    | Arg::Ref(..) => return Ok(None),
    // We do support v8 type arguments (including Option<...>)
    Arg::V8Ref(RefType::Ref, _)
    | Arg::V8Local(_)
    | Arg::OptionV8Local(_)
    | Arg::OptionV8Ref(RefType::Ref, _) => V8FastCallType::V8Value,

    Arg::Numeric(NumericArg::bool, _) => V8FastCallType::Bool,
    Arg::Numeric(NumericArg::u32, _)
    | Arg::Numeric(NumericArg::u16, _)
    | Arg::Numeric(NumericArg::u8, _) => V8FastCallType::U32,
    Arg::Numeric(NumericArg::i32, _)
    | Arg::Numeric(NumericArg::i16, _)
    | Arg::Numeric(NumericArg::i8, _)
    | Arg::Numeric(NumericArg::__SMI__, _) => V8FastCallType::I32,
    Arg::Numeric(NumericArg::u64 | NumericArg::usize, NumericFlag::None) => {
      V8FastCallType::U64
    }
    Arg::Numeric(NumericArg::i64 | NumericArg::isize, NumericFlag::None) => {
      V8FastCallType::I64
    }
    Arg::Numeric(
      NumericArg::u64 | NumericArg::usize | NumericArg::i64 | NumericArg::isize,
      NumericFlag::Number,
    ) => V8FastCallType::F64,
    Arg::Numeric(NumericArg::f32, _) => V8FastCallType::F32,
    Arg::Numeric(NumericArg::f64, _) => V8FastCallType::F64,
    // Ref strings that are one byte internally may be passed as a SeqOneByteString,
    // which gives us a FastApiOneByteString.
    Arg::String(Strings::RefStr) => V8FastCallType::SeqOneByteString,
    // Owned strings can be fast, but we'll have to copy them.
    Arg::String(Strings::String) => V8FastCallType::SeqOneByteString,
    // Cow strings can be fast, but may require copying
    Arg::String(Strings::CowStr) => V8FastCallType::SeqOneByteString,
    // Cow byte strings can be fast and don't require copying
    Arg::String(Strings::CowByte) => V8FastCallType::SeqOneByteString,
    Arg::External(..) => V8FastCallType::Pointer,
    Arg::CppGcResource(..) => V8FastCallType::V8Value,
    Arg::OptionCppGcResource(..) => V8FastCallType::V8Value,
    _ => return Err("a fast argument"),
  };
  Ok(Some(rv))
}

fn map_retval_to_v8_fastcall_type(
  arg: &Arg,
) -> Result<Option<V8FastCallType>, V8MappingError> {
  let rv = match arg {
    Arg::OptionNumeric(..)
    | Arg::SerdeV8(_)
    | Arg::ToV8(_)
    | Arg::WebIDL(_, _, _) => return Ok(None),
    Arg::VoidUndefined | Arg::Void => V8FastCallType::Void,
    Arg::Numeric(NumericArg::bool, _) => V8FastCallType::Bool,
    Arg::Numeric(NumericArg::u32, _)
    | Arg::Numeric(NumericArg::u16, _)
    | Arg::Numeric(NumericArg::u8, _) => V8FastCallType::U32,
    Arg::Numeric(NumericArg::__SMI__, _)
    | Arg::Numeric(NumericArg::i32, _)
    | Arg::Numeric(NumericArg::i16, _)
    | Arg::Numeric(NumericArg::i8, _) => V8FastCallType::I32,
    Arg::Numeric(NumericArg::u64 | NumericArg::usize, NumericFlag::None) => {
      V8FastCallType::U64
    }
    Arg::Numeric(NumericArg::i64 | NumericArg::isize, NumericFlag::None) => {
      V8FastCallType::I64
    }
    Arg::Numeric(
      NumericArg::u64 | NumericArg::usize | NumericArg::i64 | NumericArg::isize,
      NumericFlag::Number,
    ) => V8FastCallType::F64,
    Arg::Numeric(NumericArg::f32, _) => V8FastCallType::F32,
    Arg::Numeric(NumericArg::f64, _) => V8FastCallType::F64,
    // We don't return special return types
    Arg::Option(_) => return Ok(None),
    Arg::OptionString(_) => return Ok(None),
    Arg::Special(_) => return Ok(None),
    Arg::String(_) => return Ok(None),
    // We don't support returning v8 types
    Arg::V8Ref(..)
    | Arg::V8Local(_)
    | Arg::OptionV8Local(_)
    | Arg::OptionV8Ref(..)
    | Arg::CppGcResource(..)
    | Arg::OptionCppGcResource(..) => return Ok(None),
    Arg::Buffer(..) | Arg::OptionBuffer(..) => return Ok(None),
    Arg::External(..) => V8FastCallType::Pointer,
    _ => return Err("a fast return value"),
  };
  Ok(Some(rv))
}
