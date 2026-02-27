// Copyright 2018-2025 the Deno authors. MIT license.

use super::V8MappingError;
use super::V8SignatureMappingError;
use super::config::MacroConfig;
use super::dispatch_slow::generate_dispatch_slow_call;
use super::dispatch_slow::return_value_infallible;
use super::dispatch_slow::return_value_result;
use super::dispatch_slow::return_value_v8_value;
use super::dispatch_slow::throw_exception;
use super::dispatch_slow::with_fn_args;
use super::dispatch_slow::with_opctx;
use super::dispatch_slow::with_opstate;
use super::dispatch_slow::with_required_check;
use super::dispatch_slow::with_retval;
use super::dispatch_slow::with_scope;
use super::dispatch_slow::with_self;
use super::dispatch_slow::with_stack_trace;
use super::generator_state::GeneratorState;
use super::generator_state::gs_quote;
use super::signature::ParsedSignature;
use super::signature_retval::RetVal;
use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn map_async_return_type(
  generator_state: &mut GeneratorState,
  ret_val: &RetVal,
) -> Result<(TokenStream, TokenStream, TokenStream), V8MappingError> {
  let return_value = return_value_v8_value(generator_state, ret_val.arg())?;

  let fut = if generator_state.is_fake_async {
    Some(ret_val)
  } else {
    ret_val.get_future()
  };

  let (mapper, return_value_immediate) = if let Some(ret_val) = fut {
    if let Some(res_ret_val) = ret_val.unwrap_result() {
      (
        quote!(map_async_op_fallible),
        return_value_result(generator_state, res_ret_val.arg())?,
      )
    } else {
      (
        quote!(map_async_op_infallible),
        return_value_infallible(generator_state, ret_val.arg())?,
      )
    }
  } else {
    return Err("an async return");
  };

  Ok((return_value, mapper, return_value_immediate))
}

pub(crate) fn generate_dispatch_async(
  config: &MacroConfig,
  generator_state: &mut GeneratorState,
  signature: &ParsedSignature,
) -> Result<TokenStream, V8SignatureMappingError> {
  let mut output = TokenStream::new();

  let with_self = if generator_state.needs_self {
    with_self(generator_state, &signature.ret_val)
  } else {
    quote!()
  };

  // Set input_index = 1 when we don't want promise ID as the first arg.
  let input_index = if config.promise_id { 0 } else { 1 };
  let args =
    generate_dispatch_slow_call(generator_state, signature, input_index)?;

  // Always need context and args
  generator_state.needs_opctx = true;
  generator_state.needs_args = true;

  // We don't have an isolate-only fast path for async yet
  generator_state.needs_scope |= generator_state.needs_isolate;

  let (return_value, mapper, return_value_immediate) =
    map_async_return_type(generator_state, &signature.ret_val).map_err(
      |s| {
        V8SignatureMappingError::NoRetValMapping(
          signature.ret_span,
          s,
          Box::new(signature.ret_val.clone()),
        )
      },
    )?;

  output.extend(gs_quote!(generator_state(result) => {
    let #result = {
      #args
    };
  }));

  // TODO(mmastrac): we should save this unwrapped result
  if signature.ret_val.unwrap_result().is_some()
    && !generator_state.is_fake_async
  {
    let exception = throw_exception(generator_state);
    output.extend(gs_quote!(generator_state(result) => {
      let #result = match #result {
        Ok(#result) => #result,
        Err(err) => {
          // Handle eager error -- this will leave only a Future<R> or Future<Result<R>>
          #exception
        }
      };
    }));
  }

  if config.async_lazy || config.async_deferred {
    let lazy = config.async_lazy;
    let deferred = config.async_deferred;
    output.extend(gs_quote!(generator_state(promise_id, fn_args, result, opctx, scope) => {
      let #promise_id = deno_core::_ops::to_i32_option(&#fn_args.get(0)).unwrap_or_default();
      // Lazy and deferred results will always return None
      deno_core::_ops::#mapper(#opctx, #lazy, #deferred, #promise_id, #result, |#scope, #result| {
        #return_value
      });
    }));
  } else {
    output.extend(gs_quote!(generator_state(promise_id, fn_args, result, opctx, scope) => {
      let #promise_id = deno_core::_ops::to_i32_option(&#fn_args.get(0)).unwrap_or_default();
      if let Some(#result) = deno_core::_ops::#mapper(#opctx, false, false, #promise_id, #result, |#scope, #result| {
        #return_value
      }) {
        // Eager poll returned a value
        #return_value_immediate;
        return 0;
      }
    }));
  }
  output.extend(quote!(return 2;));

  let with_opstate =
    if generator_state.needs_opstate | generator_state.needs_stack_trace {
      with_opstate(generator_state)
    } else {
      quote!()
    };

  let with_opctx =
    if generator_state.needs_opctx | generator_state.needs_stack_trace {
      with_opctx(generator_state)
    } else {
      quote!()
    };

  let with_retval = if generator_state.needs_retval {
    with_retval(generator_state)
  } else {
    quote!()
  };

  let with_args = if generator_state.needs_args {
    with_fn_args(generator_state)
  } else {
    quote!()
  };

  let with_required_check = if generator_state.needs_args
    && let Some(required) = config.required
    && required > 0
  {
    with_required_check(generator_state, required, true)
  } else {
    quote!()
  };

  let with_scope =
    if generator_state.needs_scope | generator_state.needs_stack_trace {
      with_scope(generator_state)
    } else {
      quote!()
    };

  let with_stack_trace = if generator_state.needs_stack_trace {
    with_stack_trace(generator_state)
  } else {
    quote!()
  };

  Ok(
    gs_quote!(generator_state(info, slow_function, slow_function_metrics, opctx) => {
      fn slow_function_impl<'s>(info: &'s deno_core::v8::FunctionCallbackInfo) -> usize {
        #[cfg(debug_assertions)]
        let _reentrancy_check_guard = deno_core::_ops::reentrancy_check(&<Self as deno_core::_ops::Op>::DECL);

        #with_scope
        #with_retval
        #with_args
        #with_required_check
        #with_opctx
        #with_opstate
        #with_self
        #with_stack_trace

        #output
      }

      extern "C" fn #slow_function<'s>(#info: *const deno_core::v8::FunctionCallbackInfo) {
        let info: &'s _ = unsafe { &*#info };
        Self::slow_function_impl(info);
      }

      extern "C" fn #slow_function_metrics<'s>(#info: *const deno_core::v8::FunctionCallbackInfo) {
        let info: &'s _ = unsafe { &*#info };
        let args = deno_core::v8::FunctionCallbackArguments::from_function_callback_info(info);
        let #opctx: &'s _ = unsafe {
          &*(deno_core::v8::Local::<deno_core::v8::External>::cast_unchecked(args.data()).value()
            as *const deno_core::_ops::OpCtx)
        };
        deno_core::_ops::dispatch_metrics_async(#opctx, deno_core::_ops::OpMetricsEvent::Dispatched);
        let res = Self::slow_function_impl(info);
        if res == 0 {
          deno_core::_ops::dispatch_metrics_async(#opctx, deno_core::_ops::OpMetricsEvent::Completed);
        } else if res == 1 {
          deno_core::_ops::dispatch_metrics_async(#opctx, deno_core::_ops::OpMetricsEvent::Error);
        }
      }
    }),
  )
}
