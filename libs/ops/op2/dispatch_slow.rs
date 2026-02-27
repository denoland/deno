// Copyright 2018-2025 the Deno authors. MIT license.

use crate::op2::signature::AttributeModifier;

use super::V8MappingError;
use super::V8SignatureMappingError;
use super::config::MacroConfig;
use super::dispatch_shared::v8_intermediate_to_arg;
use super::dispatch_shared::v8_to_arg;
use super::dispatch_shared::v8slice_to_buffer;
use super::generator_state::GeneratorState;
use super::generator_state::gs_extract;
use super::generator_state::gs_quote;
use super::signature::Arg;
use super::signature::ArgMarker;
use super::signature::ArgSlowRetval;
use super::signature::Attributes;
use super::signature::BufferMode;
use super::signature::BufferSource;
use super::signature::BufferType;
use super::signature::External;
use super::signature::NumericArg;
use super::signature::NumericFlag;
use super::signature::ParsedSignature;
use super::signature::RefType;
use super::signature::Special;
use super::signature::Strings;
use super::signature::WebIDLPair;
use super::signature_retval::RetVal;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::format_ident;
use quote::quote;
use syn::parse2;

pub(crate) fn generate_dispatch_slow_call(
  generator_state: &mut GeneratorState,
  signature: &ParsedSignature,
  mut input_index: usize,
) -> Result<TokenStream, V8SignatureMappingError> {
  // Collect virtual arguments in a deferred list that we compute at the very end. This allows us to borrow
  // the scope/opstate in the intermediate stages.
  let mut args = TokenStream::new();
  let mut deferred = TokenStream::new();

  for (index, (arg, attrs)) in signature.args.iter().enumerate() {
    let arg_span = signature
      .arg_spans
      .get(index)
      .copied()
      .unwrap_or_else(Span::call_site);
    let arg_mapped = from_arg(generator_state, index, arg, &signature.ret_val)
      .map_err(|s| {
        V8SignatureMappingError::NoArgMapping(
          arg_span,
          s,
          Box::new(arg.clone()),
        )
      })?;
    if arg.is_virtual() {
      deferred.extend(arg_mapped);
    } else {
      args.extend(extract_arg(generator_state, attrs, index, input_index));
      args.extend(arg_mapped);
      input_index += 1;
    }
  }

  args.extend(deferred);
  args.extend(call(generator_state, &signature.ret_val));
  Ok(args)
}

pub(crate) fn generate_dispatch_slow(
  config: &MacroConfig,
  generator_state: &mut GeneratorState,
  signature: &ParsedSignature,
) -> Result<TokenStream, V8SignatureMappingError> {
  let mut output = TokenStream::new();

  let args = generate_dispatch_slow_call(generator_state, signature, 0)?;

  output.extend(gs_quote!(generator_state(result) => (let #result = {
    #args
  };)));
  if !config.setter {
    output.extend(return_value(generator_state, &signature.ret_val).map_err(
      |s| {
        V8SignatureMappingError::NoRetValMapping(
          signature.ret_span,
          s,
          Box::new(signature.ret_val.clone()),
        )
      },
    )?);
  }

  let with_validate = if let Some(validate) = &config.validate {
    generator_state.needs_scope = true;
    generator_state.needs_args = true;

    let exception = throw_exception(generator_state);
    gs_quote!(generator_state(scope, fn_args) => {
      if let Err(err) = #validate(&mut #scope, &#fn_args) {
        #exception;
      }
    })
  } else {
    quote!()
  };

  let with_stack_trace = if generator_state.needs_stack_trace {
    with_stack_trace(generator_state)
  } else {
    quote!()
  };

  // We only generate the isolate if we need it but don't need a scope. We call it `scope`.
  let with_isolate =
    if generator_state.needs_isolate && !generator_state.needs_scope {
      with_isolate(generator_state)
    } else {
      quote!()
    };

  let with_opstate = if generator_state.needs_opstate {
    with_opstate(generator_state)
  } else {
    quote!()
  };

  let with_js_runtime_state = if generator_state.needs_js_runtime_state {
    with_js_runtime_state(generator_state)
  } else {
    quote!()
  };

  let with_opctx = if generator_state.needs_opctx {
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
    with_required_check(generator_state, required, false)
  } else {
    quote!()
  };

  let with_self = if generator_state.needs_self {
    with_self(generator_state, &signature.ret_val)
  } else {
    quote!()
  };

  let with_scope = if generator_state.needs_scope {
    with_scope(generator_state)
  } else {
    quote!()
  };

  Ok(
    gs_quote!(generator_state(opctx, info, slow_function, slow_function_metrics) => {
      fn slow_function_impl<'s>(#info: &'s deno_core::v8::FunctionCallbackInfo) -> usize {
        #[cfg(debug_assertions)]
        let _reentrancy_check_guard = deno_core::_ops::reentrancy_check(&<Self as deno_core::_ops::Op>::DECL);

        #with_scope
        #with_retval
        #with_args
        #with_validate
        #with_required_check
        #with_opctx
        #with_self
        #with_isolate
        #with_opstate
        #with_stack_trace
        #with_js_runtime_state

        #output;
        return 0;
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

        deno_core::_ops::dispatch_metrics_slow(#opctx, deno_core::_ops::OpMetricsEvent::Dispatched);
        let res = Self::slow_function_impl(info);
        if res == 0 {
          deno_core::_ops::dispatch_metrics_slow(#opctx, deno_core::_ops::OpMetricsEvent::Completed);
        } else {
          deno_core::_ops::dispatch_metrics_slow(#opctx, deno_core::_ops::OpMetricsEvent::Error);
        }
      }
    }),
  )
}

pub(crate) fn with_isolate(
  generator_state: &mut GeneratorState,
) -> TokenStream {
  generator_state.needs_opctx = true;
  gs_quote!(generator_state(opctx, scope) =>
    (let mut #scope = unsafe { deno_core::v8::Isolate::from_raw_isolate_ptr(#opctx.isolate) };
    let mut scope = &mut #scope;)
  )
}

pub(crate) fn with_scope(generator_state: &mut GeneratorState) -> TokenStream {
  gs_quote!(generator_state(info, scope) =>
    (let #scope = ::std::pin::pin!(unsafe { deno_core::v8::CallbackScope::new(#info) }); let mut scope = scope.init();)
  )
}

pub(crate) fn with_stack_trace(
  generator_state: &mut GeneratorState,
) -> TokenStream {
  generator_state.needs_opctx = true;
  generator_state.needs_opstate = true;
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
}

pub(crate) fn with_retval(generator_state: &mut GeneratorState) -> TokenStream {
  gs_quote!(generator_state(retval, info) =>
    (let mut #retval = deno_core::v8::ReturnValue::from_function_callback_info(#info);)
  )
}

pub(crate) fn with_fn_args(
  generator_state: &mut GeneratorState,
) -> TokenStream {
  gs_quote!(generator_state(info, fn_args) =>
    (let #fn_args = deno_core::v8::FunctionCallbackArguments::from_function_callback_info(#info);)
  )
}

pub(crate) fn get_prefix(generator_state: &mut GeneratorState) -> String {
  if generator_state.needs_self {
    format!(
      "Failed to execute '{}' on '{}'",
      generator_state.name, generator_state.self_ty
    )
  } else if generator_state.use_this_cppgc {
    format!("Failed to construct '{}'", generator_state.self_ty)
  } else {
    format!(
      "Failed to execute '{}.{}'",
      generator_state.self_ty, generator_state.name
    )
  }
}

pub(crate) fn with_required_check(
  generator_state: &mut GeneratorState,
  required: u8,
  is_async: bool,
) -> TokenStream {
  generator_state.needs_scope = true;
  let arguments_lit = if required > 1 {
    "arguments"
  } else {
    "argument"
  };

  let prefix = get_prefix(generator_state);

  let async_offset = if is_async { 1 } else { 0 };

  gs_quote!(generator_state(fn_args, scope) =>
    (if #fn_args.length() < (#required as i32) + #async_offset {
      let msg = format!(
        "{}: {} {} required, but only {} present",
        #prefix,
        #required,
        #arguments_lit,
        #fn_args.length() - #async_offset
      );
      let msg = deno_core::v8::String::new(&mut #scope, &msg).unwrap();
      let exception = deno_core::v8::Exception::type_error(&mut #scope, msg.into());
      #scope.throw_exception(exception);
      return 1;
    })
  )
}

pub(crate) fn with_opctx(generator_state: &mut GeneratorState) -> TokenStream {
  generator_state.needs_args = true;
  gs_quote!(generator_state(opctx, fn_args) =>
    (let #opctx: &'s _ = unsafe {
    &*(deno_core::v8::Local::<deno_core::v8::External>::cast_unchecked(#fn_args.data()).value()
        as *const deno_core::_ops::OpCtx)
    };)
  )
}

pub(crate) fn with_opstate(
  generator_state: &mut GeneratorState,
) -> TokenStream {
  generator_state.needs_opctx = true;
  gs_quote!(generator_state(opctx, opstate) =>
    (let #opstate = &#opctx.state;)
  )
}

pub(crate) fn with_js_runtime_state(
  generator_state: &mut GeneratorState,
) -> TokenStream {
  generator_state.needs_opctx = true;
  gs_quote!(generator_state(opctx, js_runtime_state) =>
    (let #js_runtime_state = &#opctx.runtime_state();)
  )
}

pub(crate) fn with_self(
  generator_state: &mut GeneratorState,
  ret_val: &RetVal,
) -> TokenStream {
  generator_state.needs_opctx = true;
  generator_state.needs_scope = true;
  let throw_exception = throw_type_error(
    generator_state,
    format!("expected {}", &generator_state.self_ty),
  );
  if ret_val.is_async() {
    let tokens = gs_quote!(generator_state(self_ty, fn_args, scope, try_unwrap_cppgc) => {
      let Some(mut self_) = deno_core::_ops::#try_unwrap_cppgc::<#self_ty>(&mut #scope, #fn_args.this().into()) else {
        #throw_exception;
      };
      self_.root();
    });

    generator_state.moves.push(quote! {
      let self_ = unsafe { self_.as_ref() };
    });

    tokens
  } else {
    gs_quote!(generator_state(self_ty, fn_args, scope, try_unwrap_cppgc) => {
      let Some(self_) = deno_core::_ops::#try_unwrap_cppgc::<#self_ty>(&mut #scope, #fn_args.this().into()) else {
        #throw_exception;
      };
      let self_ = unsafe { self_.as_ref() };
    })
  }
}

pub fn extract_arg(
  generator_state: &mut GeneratorState,
  attrs: &Attributes,
  index: usize,
  input_index: usize,
) -> TokenStream {
  let exception = throw_exception(generator_state);
  let arg_ident = generator_state.args.get(index);

  let mut early_validate = quote! {};

  for attr in &attrs.rest {
    if let AttributeModifier::Validate(path) = attr {
      generator_state.needs_scope = true;
      let scope = &generator_state.scope;

      early_validate = quote! {
        match #path(&mut #scope, #arg_ident) {
          Ok(_) => {}
          Err(err) => {
            #exception;
          }
        };
      };
    }
  }

  let fn_args = &generator_state.fn_args;
  quote!(
    let #arg_ident = #fn_args.get(#input_index as i32);
    #early_validate
  )
}

pub fn from_arg(
  mut generator_state: &mut GeneratorState,
  index: usize,
  arg: &Arg,
  ret_val: &RetVal,
) -> Result<TokenStream, V8MappingError> {
  let GeneratorState {
    args,
    scope,
    opstate,
    opctx,
    js_runtime_state,
    needs_scope,
    needs_isolate,
    needs_opstate,
    needs_opctx,
    fn_args,
    needs_js_runtime_state,
    ..
  } = &mut generator_state;
  let arg_ident = args
    .get(index)
    .expect("Argument at index was missing")
    .clone();
  let arg_temp = format_ident!("{}_temp", arg_ident);
  let res = match arg {
    Arg::Numeric(NumericArg::bool, _) => quote! {
      let #arg_ident = #arg_ident.is_true();
    },
    Arg::Numeric(NumericArg::u8, _)
    | Arg::Numeric(NumericArg::u16, _)
    | Arg::Numeric(NumericArg::u32, _) => {
      from_arg_option(generator_state, &arg_ident, "u32")
    }
    Arg::Numeric(NumericArg::i8, _)
    | Arg::Numeric(NumericArg::i16, _)
    | Arg::Numeric(NumericArg::i32, _)
    | Arg::Numeric(NumericArg::__SMI__, _) => {
      from_arg_option(generator_state, &arg_ident, "i32")
    }
    Arg::Numeric(NumericArg::u64 | NumericArg::usize, NumericFlag::None) => {
      from_arg_option(generator_state, &arg_ident, "u64")
    }
    Arg::Numeric(NumericArg::i64 | NumericArg::isize, NumericFlag::None) => {
      from_arg_option(generator_state, &arg_ident, "i64")
    }
    Arg::Numeric(
      NumericArg::u64 | NumericArg::usize | NumericArg::i64 | NumericArg::isize,
      NumericFlag::Number,
    ) => from_arg_option(generator_state, &arg_ident, "f64"),
    Arg::Numeric(NumericArg::f32, _) => {
      from_arg_option(generator_state, &arg_ident, "f32")
    }
    Arg::Numeric(NumericArg::f64, _) => {
      from_arg_option(generator_state, &arg_ident, "f64")
    }
    Arg::OptionNumeric(numeric, flag) => {
      let some = from_arg(
        generator_state,
        index,
        &Arg::Numeric(*numeric, *flag),
        ret_val,
      )?;
      quote! {
        let #arg_ident = if #arg_ident.is_null_or_undefined() {
          None
        } else {
          #some
          Some(#arg_ident)
        };
      }
    }
    Arg::OptionString(Strings::String) => {
      // Only requires isolate, not a full scope
      *needs_isolate = true;
      quote! {
        let #arg_ident = if #arg_ident.is_null_or_undefined() {
          None
        } else {
          Some(deno_core::_ops::to_string(&mut #scope, &#arg_ident))
        };
      }
    }
    Arg::String(Strings::String) => {
      // Only requires isolate, not a full scope
      *needs_isolate = true;

      quote! {
        let #arg_ident = deno_core::_ops::to_string(&mut #scope, &#arg_ident);
      }
    }
    Arg::String(Strings::RefStr) | Arg::String(Strings::CowStr) => {
      // Only requires isolate, not a full scope
      *needs_isolate = true;
      let maybe_scope = if *needs_scope {
        quote!()
      } else {
        with_scope(generator_state)
      };

      let maybe_ref = if matches!(arg, Arg::String(Strings::RefStr)) {
        quote!(&)
      } else {
        quote!()
      };

      gs_quote!(generator_state(scope) => {
        // Trade stack space for potentially non-allocating strings
        let mut #arg_temp: [::std::mem::MaybeUninit<u8>; deno_core::_ops::STRING_STACK_BUFFER_SIZE] = [::std::mem::MaybeUninit::uninit(); deno_core::_ops::STRING_STACK_BUFFER_SIZE];
        let #arg_ident = if !#arg_ident.is_string() {
            #maybe_scope
            let tc = ::std::pin::pin!(deno_core::v8::TryCatch::new(&mut #scope));
            let mut tc = tc.init();
            match #arg_ident.to_string(&mut tc) {
                Some(v) => v.into(),
                None => {
                    tc.rethrow();
                    return 1;
                }
            }
        } else {
            #arg_ident
        };

        let #arg_ident = #maybe_ref deno_core::_ops::to_str(&mut #scope, &#arg_ident, &mut #arg_temp);
      })
    }
    Arg::String(Strings::CowByte) => {
      // Only requires isolate, not a full scope
      *needs_isolate = true;
      let throw_exception =
        throw_type_error_static_string(generator_state, &arg_ident);
      gs_quote!(generator_state(scope) => {
        // Trade stack space for potentially non-allocating strings
        let #arg_ident = match deno_core::_ops::to_cow_one_byte(&mut #scope, &#arg_ident) {
          Ok(#arg_ident) => #arg_ident,
          Err(#arg_ident) => {
            #throw_exception
          }
        };
      })
    }
    Arg::VarArgs => {
      gs_quote!(generator_state(fn_args) => {
        let #arg_ident = Some(&#fn_args);
      })
    }
    Arg::This => {
      *needs_isolate = true;
      gs_quote!(generator_state(scope, fn_args) => {
        let #arg_ident = deno_core::v8::Global::new(&mut #scope, #fn_args.this());
      })
    }
    Arg::Buffer(buffer_type, mode, source) => {
      // Explicit temporary lifetime extension so we can take a reference
      let temp = format_ident!("{}_temp", arg_ident);
      let buffer = from_arg_array_or_buffer(
        generator_state,
        &arg_ident,
        *buffer_type,
        *mode,
        *source,
        &temp,
      )?;
      quote! {
        let mut #temp;
        #buffer
      }
    }
    Arg::OptionBuffer(buffer_type, mode, source) => {
      // Explicit temporary lifetime extension so we can take a reference
      let temp = format_ident!("{}_temp", arg_ident);
      let some = from_arg_array_or_buffer(
        generator_state,
        &arg_ident,
        *buffer_type,
        *mode,
        *source,
        &temp,
      )?;
      quote! {
        let mut #temp;
        let #arg_ident = if #arg_ident.is_null_or_undefined() {
          None
        } else {
          #some
          Some(#arg_ident)
        };
      }
    }
    Arg::External(External::Ptr(_)) => {
      from_arg_option(generator_state, &arg_ident, "external")
    }
    Arg::Ref(RefType::Ref, Special::Isolate) => {
      *needs_opctx = true;
      quote!(
        let #arg_ident = unsafe { deno_core::v8::Isolate::from_raw_isolate_ptr(#opctx.isolate) };
        let #arg_ident = &#arg_ident;
      )
    }
    Arg::Ref(RefType::Mut, Special::Isolate) => {
      *needs_opctx = true;
      quote!(
        let mut #arg_ident = unsafe { deno_core::v8::Isolate::from_raw_isolate_ptr(#opctx.isolate) };
        let #arg_ident = &mut #arg_ident;
      )
    }
    Arg::Ref(_, Special::HandleScope) => {
      *needs_scope = true;
      quote!(let #arg_ident = &mut #scope;)
    }
    Arg::Ref(RefType::Ref, Special::OpState) => {
      *needs_opstate = true;
      quote!(let #arg_ident = &::std::cell::RefCell::borrow(&#opstate);)
    }
    Arg::Ref(RefType::Mut, Special::OpState) => {
      *needs_opstate = true;
      quote!(let #arg_ident = &mut ::std::cell::RefCell::borrow_mut(&#opstate);)
    }
    Arg::RcRefCell(Special::OpState) => {
      *needs_opstate = true;
      quote!(let #arg_ident = #opstate.clone();)
    }
    Arg::Ref(RefType::Ref, Special::JsRuntimeState) => {
      *needs_js_runtime_state = true;
      quote!(let #arg_ident = &#js_runtime_state;)
    }
    Arg::V8Local(v8)
    | Arg::OptionV8Local(v8)
    | Arg::V8Ref(RefType::Ref, v8)
    | Arg::OptionV8Ref(RefType::Ref, v8) => {
      let throw_type_error = || {
        Ok(throw_type_error(
          generator_state,
          format!("expected {v8:?}"),
        ))
      };
      let extract_intermediate = v8_intermediate_to_arg(&arg_ident, arg);
      v8_to_arg(v8, &arg_ident, arg, throw_type_error, extract_intermediate)?
    }
    Arg::SerdeV8(_class) => {
      *needs_scope = true;
      let scope = scope.clone();
      let err = format_ident!("{}_err", arg_ident);
      let throw_exception = throw_type_error_string(generator_state, &err);
      quote! {
        let #arg_ident = match deno_core::_ops::serde_v8_to_rust(&mut #scope, #arg_ident) {
          Ok(t) => t,
          Err(#err) => {
            #throw_exception;
          }
        };
      }
    }
    Arg::FromV8(ty, true) => {
      *needs_scope = true;
      let ty = syn::parse_str::<syn::Type>(ty)
        .map_err(|_| "failed to reparse type")?;
      let scope = scope.clone();
      let err = format_ident!("{}_err", arg_ident);
      let throw_exception = throw_type_error_string(generator_state, &err);
      quote! {
        let #arg_ident = {
          use deno_core::FromV8;
          use deno_core::FromV8Scopeless;

          match <#ty>::from_v8(&mut #scope, #arg_ident) {
            Ok(t) => t,
            Err(#err) => {
              #throw_exception;
            }
          }
        };
      }
    }
    Arg::FromV8(ty, false) => {
      let ty = syn::parse_str::<syn::Type>(ty)
        .map_err(|_| "failed to reparse type")?;
      let err = format_ident!("{}_err", arg_ident);
      let throw_exception = throw_type_error_string(generator_state, &err);
      quote! {
        let #arg_ident = match <#ty as deno_core::FromV8Scopeless>::from_v8(#arg_ident) {
          Ok(t) => t,
          Err(#err) => {
            #throw_exception;
          }
        };
      }
    }
    Arg::WebIDL(ty, options, default) => {
      *needs_scope = true;
      let ty = syn::parse_str::<syn::Type>(ty)
        .map_err(|_| "failed to reparse type")?;
      let scope = scope.clone();
      let err = format_ident!("{}_err", arg_ident);
      let throw_exception = throw_type_error_string(generator_state, &err);
      let prefix = get_prefix(generator_state);
      let context = format!("Argument {}", index);

      let (alias, options) = if options.is_empty() {
        (None, quote!(Default::default()))
      } else {
        let inner = options
          .iter()
          .map(|WebIDLPair(k, v)| quote!(#k: #v))
          .collect::<Vec<_>>();

        let alias = format_ident!("{arg_ident}_webidl_alias");
        // Type-alias to workaround https://github.com/rust-lang/rust/issues/86935
        (
          Some(quote! {
            type #alias<'a> = <#ty as ::deno_core::webidl::WebIdlConverter<'a>>::Options;
          }),
          quote! {
            #alias {
              #(#inner),*,
              ..Default::default()
            }
          },
        )
      };

      let default = if let Some(default) = default {
        let tokens = default.0.to_token_stream();
        let default = match parse2::<syn::LitStr>(tokens.clone()) {
          Ok(lit) => {
            if lit.value().is_empty() {
              quote! {
                deno_core::v8::String::empty(&mut #scope)
              }
            } else {
              return Err("unsupported WebIDL default value");
            }
          }
          _ => match parse2::<syn::LitInt>(tokens.clone()) {
            Ok(lit) => {
              quote! {
                deno_core::v8::Number::new(&mut #scope, #lit as _)
              }
            }
            _ => match parse2::<syn::LitFloat>(tokens) {
              Ok(lit) => {
                quote! {
                  deno_core::v8::Number::new(&mut #scope, #lit)
                }
              }
              _ => {
                return Err("unsupported WebIDL default value");
              }
            },
          },
        };

        quote! {
          let #arg_ident = if #arg_ident.is_undefined() {
            #default.into()
          } else {
            #arg_ident
          };
        }
      } else {
        quote!()
      };

      quote! {
        #default
        #alias
        let #arg_ident = match <#ty as deno_core::webidl::WebIdlConverter>::convert(
          &mut #scope,
          #arg_ident,
          #prefix.into(),
          deno_core::webidl::ContextFn::new_borrowed(&|| std::borrow::Cow::Borrowed(#context)),
          &#options,
        ) {
          Ok(t) => t,
          Err(#err) => {
            #throw_exception;
          }
        };
      }
    }
    Arg::CppGcResource(proto, ty) => {
      *needs_scope = true;
      let from_ident = if *proto {
        quote!(#fn_args.this().into())
      } else {
        quote!(#arg_ident)
      };
      let throw_exception =
        throw_type_error(generator_state, format!("expected {}", &ty));

      let scope = &generator_state.scope;
      let try_unwrap_cppgc = &generator_state.try_unwrap_cppgc;
      let ty = syn::parse_str::<syn::Path>(ty)
        .map_err(|_| "failed to reparse type")?;
      if ret_val.is_async() {
        let tokens = quote! {
          let Some(mut #arg_ident) = deno_core::_ops::#try_unwrap_cppgc::<#ty>(&mut #scope, #from_ident) else {
            #throw_exception;
          };
          #arg_ident.root();
        };
        generator_state.moves.push(quote! {
          let #arg_ident = unsafe { #arg_ident.as_ref() };
        });
        tokens
      } else {
        quote! {
          let Some(#arg_ident) = deno_core::_ops::#try_unwrap_cppgc::<#ty>(&mut #scope, #from_ident) else {
            #throw_exception;
          };
          let #arg_ident = unsafe { #arg_ident.as_ref() };
        }
      }
    }
    Arg::OptionCppGcResource(ty) => {
      *needs_scope = true;
      let throw_exception =
        throw_type_error(generator_state, format!("expected {}", &ty));
      let ty = syn::parse_str::<syn::Path>(ty)
        .map_err(|_| "failed to reparse type")?;
      let scope = &generator_state.scope;
      let try_unwrap_cppgc = &generator_state.try_unwrap_cppgc;
      if ret_val.is_async() {
        let tokens = quote! {
          let #arg_ident = if #arg_ident.is_null_or_undefined() {
            None
          } else if let Some(mut #arg_ident) = deno_core::_ops::#try_unwrap_cppgc::<#ty>(&mut #scope, #arg_ident) {
            #arg_ident.root();
            Some(#arg_ident)
          } else {
            #throw_exception;
          };
        };

        generator_state.moves.push(quote! {
          let #arg_ident = unsafe { #arg_ident.as_ref().map(|a| a.as_ref()) };
        });

        tokens
      } else {
        quote! {
          let #arg_ident = if #arg_ident.is_null_or_undefined() {
            None
          } else if let Some(#arg_ident) = deno_core::_ops::try_unwrap_cppgc_object::<#ty>(&mut #scope, #arg_ident) {
            Some(#arg_ident)
          } else {
            #throw_exception;
          };
          let #arg_ident = unsafe { #arg_ident.as_ref().map(|a| a.as_ref()) };
        }
      }
    }
    _ => return Err("a slow argument"),
  };
  Ok(res)
}

/// Converts an argument using a simple `to_XXX_option`-style method.
pub fn from_arg_option(
  generator_state: &mut GeneratorState,
  arg_ident: &Ident,
  numeric: &str,
) -> TokenStream {
  let exception =
    throw_type_error(generator_state, format!("expected {numeric}"));
  let convert = format_ident!("to_{numeric}_option");
  quote!(
    let Some(#arg_ident) = deno_core::_ops::#convert(&#arg_ident) else {
      #exception
    };
    let #arg_ident = #arg_ident as _;
  )
}

pub fn from_arg_array_or_buffer(
  generator_state: &mut GeneratorState,
  arg_ident: &Ident,
  buffer_type: BufferType,
  buffer_mode: BufferMode,
  buffer_source: BufferSource,
  temp: &Ident,
) -> Result<TokenStream, V8MappingError> {
  match buffer_source {
    BufferSource::TypedArray => from_arg_buffer(
      generator_state,
      arg_ident,
      buffer_type,
      buffer_mode,
      temp,
    ),
    BufferSource::ArrayBuffer => from_arg_arraybuffer(
      generator_state,
      arg_ident,
      buffer_type,
      buffer_mode,
      temp,
    ),
    BufferSource::Any => from_arg_any_buffer(
      generator_state,
      arg_ident,
      buffer_type,
      buffer_mode,
      temp,
    ),
  }
}

pub fn from_arg_buffer(
  generator_state: &mut GeneratorState,
  arg_ident: &Ident,
  buffer_type: BufferType,
  buffer_mode: BufferMode,
  temp: &Ident,
) -> Result<TokenStream, V8MappingError> {
  let err = format_ident!("{}_err", arg_ident);
  let throw_exception = throw_type_error_static_string(generator_state, &err);

  let array = buffer_type.element();

  let to_v8_slice = if matches!(buffer_mode, BufferMode::Detach) {
    generator_state.needs_scope = true;
    gs_quote!(generator_state(scope) => { deno_core::_ops::to_v8_slice_detachable::<#array>(&mut #scope, #arg_ident) })
  } else {
    quote!(deno_core::_ops::to_v8_slice::<#array>(#arg_ident))
  };

  let make_v8slice = quote!(
    #temp = match unsafe { #to_v8_slice } {
      Ok(#arg_ident) => #arg_ident,
      Err(#err) => {
        #throw_exception
      }
    };
  );

  let make_arg = v8slice_to_buffer(arg_ident, temp, buffer_type)?;

  Ok(quote! {
    #make_v8slice
    #make_arg
  })
}

pub fn from_arg_arraybuffer(
  generator_state: &mut GeneratorState,
  arg_ident: &Ident,
  buffer_type: BufferType,
  buffer_mode: BufferMode,
  temp: &Ident,
) -> Result<TokenStream, V8MappingError> {
  let err = format_ident!("{}_err", arg_ident);
  let throw_exception = throw_type_error_static_string(generator_state, &err);

  let to_v8_slice = if matches!(buffer_mode, BufferMode::Detach) {
    quote!(to_v8_slice_buffer_detachable)
  } else {
    quote!(to_v8_slice_buffer)
  };

  let make_v8slice = quote!(
    #temp = match unsafe { deno_core::_ops::#to_v8_slice(#arg_ident) } {
      Ok(#arg_ident) => #arg_ident,
      Err(#err) => {
        #throw_exception
      }
    };
  );

  let make_arg = v8slice_to_buffer(arg_ident, temp, buffer_type)?;

  Ok(quote! {
    #make_v8slice
    #make_arg
  })
}

pub fn from_arg_any_buffer(
  generator_state: &mut GeneratorState,
  arg_ident: &Ident,
  buffer_type: BufferType,
  buffer_mode: BufferMode,
  temp: &Ident,
) -> Result<TokenStream, V8MappingError> {
  let err = format_ident!("{}_err", arg_ident);
  let throw_exception = throw_type_error_static_string(generator_state, &err);

  let to_v8_slice = if matches!(buffer_mode, BufferMode::Detach) {
    quote!(to_v8_slice_any_detachable)
  } else {
    quote!(to_v8_slice_any)
  };

  let make_v8slice = quote!(
    #temp = match unsafe { deno_core::_ops::#to_v8_slice(#arg_ident) } {
      Ok(#arg_ident) => #arg_ident,
      Err(#err) => {
        #throw_exception
      }
    };
  );

  let make_arg = v8slice_to_buffer(arg_ident, temp, buffer_type)?;

  Ok(quote! {
    #make_v8slice
    #make_arg
  })
}

pub fn call(
  generator_state: &mut GeneratorState,
  ret_val: &RetVal,
) -> TokenStream {
  let mut tokens = TokenStream::new();
  if generator_state.needs_self {
    tokens.extend(quote!(self_,));
  }
  for arg in &generator_state.args {
    tokens.extend(quote!( #arg , ));
  }

  let name = &generator_state.name;
  let call_ = if generator_state.needs_self {
    let self_ty = &generator_state.self_ty;
    quote!(#self_ty:: #name)
  } else {
    quote!(Self:: #name)
  };

  let call = quote!(#call_ ( #tokens ));

  if ret_val.is_async() && !generator_state.moves.is_empty() {
    let mut moves = TokenStream::new();
    for m in &generator_state.moves {
      moves.extend(m.clone());
    }
    quote!(async move {
      #moves
      #call.await
    })
  } else if generator_state.is_fake_async {
    quote!(std::future::ready(#call))
  } else {
    call
  }
}

pub fn return_value(
  generator_state: &mut GeneratorState,
  ret_type: &RetVal,
) -> Result<TokenStream, V8MappingError> {
  match ret_type {
    RetVal::Value(ret_type, ..) => {
      return_value_infallible(generator_state, ret_type)
    }
    RetVal::Result(ret_type) => {
      return_value_result(generator_state, ret_type.arg())
    }
    _ => todo!(),
  }
}

pub fn return_value_infallible(
  generator_state: &mut GeneratorState,
  ret_type: &Arg,
) -> Result<TokenStream, V8MappingError> {
  // In the future we may be able to make this false for void again
  generator_state.needs_retval = true;

  let result = match ret_type.marker() {
    ArgMarker::ArrayBuffer => {
      gs_quote!(generator_state(result) => (deno_core::_ops::RustToV8Marker::<deno_core::_ops::ArrayBufferMarker, _>::from(#result)))
    }
    ArgMarker::Serde => {
      gs_quote!(generator_state(result) => (deno_core::_ops::RustToV8Marker::<deno_core::_ops::SerdeMarker, _>::from(#result)))
    }
    ArgMarker::Smi => {
      gs_quote!(generator_state(result) => (deno_core::_ops::RustToV8Marker::<deno_core::_ops::SmiMarker, _>::from(#result)))
    }
    ArgMarker::Number => {
      gs_quote!(generator_state(result) => (deno_core::_ops::RustToV8Marker::<deno_core::_ops::NumberMarker, _>::from(#result)))
    }
    ArgMarker::Cppgc if generator_state.use_this_cppgc => {
      generator_state.needs_isolate = true;
      generator_state.needs_args = true;
      gs_quote!(generator_state(result, scope, fn_args) => (
        Some(deno_core::cppgc::wrap_object(&mut #scope, #fn_args.this(), #result))
      ))
    }
    ArgMarker::Cppgc => {
      let marker = quote!(deno_core::_ops::RustToV8Marker::<deno_core::_ops::CppGcMarker, _>::from);
      if ret_type.is_option() {
        gs_quote!(generator_state(result) => (#result.map(#marker)))
      } else {
        gs_quote!(generator_state(result) => (#marker(#result)))
      }
    }
    ArgMarker::ToV8 => {
      gs_quote!(generator_state(result) => (deno_core::_ops::RustToV8Marker::<deno_core::_ops::ToV8Marker, _>::from(#result)))
    }
    ArgMarker::Undefined => {
      gs_quote!(generator_state(scope) => (deno_core::v8::undefined(&mut #scope)))
    }
    ArgMarker::None => {
      gs_quote!(generator_state(result) => (#result))
    }
  };
  let res = match ret_type.slow_retval() {
    ArgSlowRetval::RetVal => {
      gs_quote!(generator_state(retval) => (deno_core::_ops::RustToV8RetVal::to_v8_rv(#result, &mut #retval)))
    }
    ArgSlowRetval::RetValFallible => {
      generator_state.needs_scope = true;
      let err = format_ident!("{}_err", generator_state.retval);
      let throw_exception = throw_type_error_string(generator_state, &err);

      gs_quote!(generator_state(scope, retval) => (match deno_core::_ops::RustToV8Fallible::to_v8_fallible(#result, &mut #scope) {
        Ok(v) => #retval.set(v),
        Err(#err) => {
          #throw_exception
        },
      }))
    }
    ArgSlowRetval::V8Local => {
      generator_state.needs_scope = true;
      gs_quote!(generator_state(scope, retval) => (#retval.set(deno_core::_ops::RustToV8::to_v8(#result, &mut #scope))))
    }
    ArgSlowRetval::V8LocalNoScope => {
      gs_quote!(generator_state(retval) => (#retval.set(deno_core::_ops::RustToV8NoScope::to_v8(#result))))
    }
    ArgSlowRetval::V8LocalFalliable => {
      generator_state.needs_scope = true;
      let err = format_ident!("{}_err", generator_state.retval);
      let throw_exception = throw_type_error_string(generator_state, &err);

      gs_quote!(generator_state(scope, retval) => (match deno_core::_ops::RustToV8Fallible::to_v8_fallible(#result, &mut #scope) {
        Ok(v) => #retval.set(v),
        Err(#err) => {
          #throw_exception
        },
      }))
    }
    ArgSlowRetval::None => return Err("a slow return value"),
  };

  Ok(res)
}

/// Puts a typed result into a [`v8::Value`].
pub fn return_value_v8_value(
  generator_state: &GeneratorState,
  ret_type: &Arg,
) -> Result<TokenStream, V8MappingError> {
  gs_extract!(generator_state(scope, result));
  let result = match ret_type.marker() {
    ArgMarker::ArrayBuffer => {
      quote!(deno_core::_ops::RustToV8Marker::<deno_core::_ops::ArrayBufferMarker, _>::from(#result))
    }
    ArgMarker::Serde => {
      quote!(deno_core::_ops::RustToV8Marker::<deno_core::_ops::SerdeMarker, _>::from(#result))
    }
    ArgMarker::Smi => {
      quote!(deno_core::_ops::RustToV8Marker::<deno_core::_ops::SmiMarker, _>::from(#result))
    }
    ArgMarker::Number => {
      quote!(deno_core::_ops::RustToV8Marker::<deno_core::_ops::NumberMarker, _>::from(#result))
    }
    ArgMarker::Cppgc => {
      let marker = if !generator_state.use_this_cppgc {
        quote!(deno_core::_ops::RustToV8Marker::<deno_core::_ops::CppGcMarker, _>::from)
      } else {
        quote!((|x| { deno_core::cppgc::wrap_object(#scope, args.this(), x) }))
      };

      if ret_type.is_option() {
        quote!(#result.map(#marker))
      } else {
        quote!(#marker(#result))
      }
    }
    ArgMarker::ToV8 => {
      quote!(deno_core::_ops::RustToV8Marker::<deno_core::_ops::ToV8Marker, _>::from(#result))
    }
    ArgMarker::Undefined => {
      quote!(deno_core::v8::undefined(#scope).into())
    }
    ArgMarker::None => quote!(#result),
  };
  let res = match ret_type.slow_retval() {
    ArgSlowRetval::RetVal | ArgSlowRetval::V8Local => {
      quote!(Ok(deno_core::_ops::RustToV8::to_v8(#result, #scope)))
    }
    ArgSlowRetval::V8LocalNoScope => {
      quote!(Ok(deno_core::_ops::RustToV8NoScope::to_v8(#result)))
    }
    ArgSlowRetval::RetValFallible | ArgSlowRetval::V8LocalFalliable => {
      quote!(deno_core::_ops::RustToV8Fallible::to_v8_fallible(#result, #scope))
    }
    ArgSlowRetval::None => return Err("a v8 return value"),
  };
  Ok(res)
}

pub fn return_value_result(
  generator_state: &mut GeneratorState,
  ret_type: &Arg,
) -> Result<TokenStream, V8MappingError> {
  let infallible = return_value_infallible(generator_state, ret_type)?;
  let exception = throw_exception(generator_state);

  let tokens = gs_quote!(generator_state(result) => (
    match #result {
      Ok(#result) => {
        #infallible
      }
      Err(err) => {
        #exception
      }
    };
  ));
  Ok(tokens)
}

/// Generates code to throw an exception, adding required additional dependencies as needed.
pub(crate) fn throw_exception(
  generator_state: &mut GeneratorState,
) -> TokenStream {
  let maybe_scope = if generator_state.needs_scope {
    quote!()
  } else {
    with_scope(generator_state)
  };

  let maybe_opctx = if generator_state.needs_opctx {
    quote!()
  } else {
    with_opctx(generator_state)
  };

  let maybe_args = if generator_state.needs_args {
    quote!()
  } else {
    with_fn_args(generator_state)
  };

  gs_quote!(generator_state(scope) => {
    #maybe_scope
    #maybe_args
    #maybe_opctx
    let exception = deno_core::error::to_v8_error(
      &mut #scope,
      &err,
    );
    #scope.throw_exception(exception);
    return 1;
  })
}

/// Generates code to throw an exception, adding required additional dependencies as needed.
fn throw_type_error(
  generator_state: &mut GeneratorState,
  message: String,
) -> TokenStream {
  // Sanity check ASCII and a valid/reasonable message size
  debug_assert!(message.is_ascii() && message.len() < 1024);

  gs_quote!(generator_state(info) => {
    deno_core::_ops::throw_error_one_byte_info(&#info, #message);
    return 1;
  })
}

/// Generates code to throw an exception from a string variable, adding required additional dependencies as needed.
fn throw_type_error_string(
  generator_state: &mut GeneratorState,
  message: &Ident,
) -> TokenStream {
  let maybe_scope = if generator_state.needs_scope {
    quote!()
  } else {
    with_scope(generator_state)
  };

  gs_quote!(generator_state(scope) => {
    #maybe_scope
    deno_core::_ops::throw_error_js_error_class(&mut #scope, &#message);
    return 1;
  })
}

/// Generates code to throw an exception from a string variable, adding required additional dependencies as needed.
fn throw_type_error_static_string(
  generator_state: &mut GeneratorState,
  message: &Ident,
) -> TokenStream {
  gs_quote!(generator_state(info) => {
    deno_core::_ops::throw_error_one_byte_info(&#info, #message);
    return 1;
  })
}
