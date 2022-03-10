use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn op(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let func = syn::parse::<syn::ItemFn>(item).expect("expected a function");
  let name = &func.sig.ident;
  let generics = &func.sig.generics;
  let type_params = &func.sig.generics.params;
  let where_clause = &func.sig.generics.where_clause;

  // Preserve the original func as op_foo::call()
  let original_func = {
    let mut func = func.clone();
    func.sig.ident = quote::format_ident!("call");
    quote! { #func }
  };

  let inputs = &func.sig.inputs;
  let output = &func.sig.output;
  let a = match &inputs[1] {
    syn::FnArg::Typed(pat) => match *pat.pat {
      syn::Pat::Wild(_) => quote! {
        let a = ();
      },
      _ => quote! {
        let a = args.get(2);
        let a = match deno_core::serde_v8::from_v8(scope, a) {
          Ok(v) => v,
          Err(err) => {
            // Throw TypeError
            let msg = format!("Error parsing args: {}", deno_core::anyhow::Error::from(err));
            let message = deno_core::v8::String::new(scope, msg.as_ref()).unwrap();
            let exception = deno_core::v8::Exception::type_error(scope, message);
            scope.throw_exception(exception);
            return;
          }
        };
      },
    },
    _ => unreachable!(),
  };

  let b = match &inputs[2] {
    syn::FnArg::Typed(pat) => match *pat.pat {
      syn::Pat::Wild(_) => quote! {
        let b = ();
      },
      _ => quote! {
        let b = args.get(3);
        let b = match deno_core::serde_v8::from_v8(scope, b) {
          Ok(v) => v,
          Err(err) => {
            // Throw TypeError
            let msg = format!("Error parsing args: {}", deno_core::anyhow::Error::from(err));
            let message = deno_core::v8::String::new(scope, msg.as_ref()).unwrap();
            let exception = deno_core::v8::Exception::type_error(scope, message);
            scope.throw_exception(exception);
            return;
          }
        };
      },
    },
    _ => unreachable!(),
  };

  let ret = match &output {
    syn::ReturnType::Default => quote! {
      let ret = ();
    },
    syn::ReturnType::Type(_, ty) => {
      let default_ok = quote! {
        let ret = deno_core::serde_v8::to_v8(scope, v).unwrap();
        rv.set(ret);
      };

      // Optimize Result<(), Err> to skip serde_v8.
      let ok_block = match &**ty {
        syn::Type::Path(ref path) => {
          let maybe_result =
            path.path.segments.first().expect("Invalid return type.");
          if maybe_result.ident.to_string() == "Result" {
            assert!(!maybe_result.arguments.is_empty());
            match &maybe_result.arguments {
              syn::PathArguments::AngleBracketed(args) => {
                let maybe_unit = args.args.first().unwrap();
                match maybe_unit {
                  syn::GenericArgument::Type(syn::Type::Tuple(ty)) => {
                    if ty.elems.is_empty() {
                      quote! {}
                    } else {
                      default_ok
                    }
                  }
                  _ => default_ok,
                }
              }
              syn::PathArguments::None
              | syn::PathArguments::Parenthesized(..) => unreachable!(),
            }
          } else {
            default_ok
          }
        }
        _ => default_ok,
      };

      quote! {
        match result {
          Ok(v) => {
            #ok_block
          },
          Err(err) => {
            let err = deno_core::serde_v8::to_v8(
              scope,
              deno_core::OpError {
                class_name: (op_state.get_error_class_fn)(&err),
                message: err.to_string(),
                code: deno_core::error_codes::get_error_code(&err),
              },
            ).unwrap();

            rv.set(err);
          },
        };
      }
    }
  };

  let is_async = func.sig.asyncness.is_some();
  if is_async {
    TokenStream::from(quote! {
      #[allow(non_camel_case_types)]
      pub struct #name;

      impl #name {
        pub fn name() -> &'static str {
          stringify!(#name)
        }

        pub fn v8_cb #generics () -> deno_core::v8::FunctionCallback #where_clause {
          use deno_core::v8::MapFnTo;
          Self::v8_func::<#type_params>.map_fn_to()
        }

        pub fn decl #generics ()  -> (&'static str, deno_core::v8::FunctionCallback) #where_clause {
          (Self::name(), Self::v8_cb::<#type_params>())
        }

        #original_func

        pub fn v8_func #generics (
          scope: &mut deno_core::v8::HandleScope,
          args: deno_core::v8::FunctionCallbackArguments,
          mut rv: deno_core::v8::ReturnValue,
        ) #where_clause {
          use deno_core::JsRuntime;
          use deno_core::futures::FutureExt;
          use deno_core::OpCall;
          use deno_core::to_op_result;
          use deno_core::PromiseId;
          use deno_core::bindings::throw_type_error;
          use deno_core::v8;
          let op_id = unsafe { v8::Local::<v8::Integer>::cast(args.get(0)) }.value() as usize;

          let promise_id = args.get(1);
          let promise_id = v8::Local::<v8::Integer>::try_from(promise_id)
            .map(|l| l.value() as PromiseId)
            .map_err(deno_core::anyhow::Error::from);
          // Fail if promise id invalid (not an int)
          let promise_id: PromiseId = match promise_id {
            Ok(promise_id) => promise_id,
            Err(err) => {
              throw_type_error(scope, format!("invalid promise id: {}", err));
              return;
            }
          };

          #a
          #b
          #func

          let state_rc = JsRuntime::state(scope);
          let mut state = state_rc.borrow_mut();
          state.op_state.borrow().tracker.track_async(op_id);
          state.have_unpolled_ops = true;
          let op_state = state.op_state.clone();
          let op_state2 = op_state.clone();
          state.pending_ops.push(OpCall::eager(async move {
            let result = #name::<#type_params>(op_state.clone(), a, b).await;
            (promise_id, op_id, to_op_result(&op_state.borrow(), result))
          }));
        }
      }
    })
  } else {
    TokenStream::from(quote! {
      #[allow(non_camel_case_types)]
      pub struct #name;

      impl #name {
        pub fn name() -> &'static str {
          stringify!(#name)
        }

        pub fn v8_cb #generics () -> deno_core::v8::FunctionCallback #where_clause {
          use deno_core::v8::MapFnTo;
          Self::v8_func::<#type_params>.map_fn_to()
        }

        pub fn decl #generics ()  -> (&'static str, deno_core::v8::FunctionCallback) #where_clause {
          (Self::name(), Self::v8_cb::<#type_params>())
        }

        #original_func

        #[inline]
        pub fn v8_func #generics (
          scope: &mut deno_core::v8::HandleScope,
          args: deno_core::v8::FunctionCallbackArguments,
          mut rv: deno_core::v8::ReturnValue,
        ) #where_clause {
          // SAFETY: Called from Deno.core.opSync. Which retrieves the index using opId table.
          let op_id = unsafe {
            deno_core::v8::Local::<deno_core::v8::Integer>::cast(args.get(0)).value()
          } as usize;

          #a
          #b
          #func

          // SAFETY: Unchecked cast to external since deno_core guarantees args.data() is a v8 External.
          let state_refcell_raw = unsafe {
            deno_core::v8::Local::<deno_core::v8::External>::cast(args.data().unwrap_unchecked())
          }.value();

          // SAFETY: The Rc<RefCell<OpState>> is functionally pinned and is tied to the isolate's lifetime
          let state = unsafe { &*(state_refcell_raw as *const std::cell::RefCell<deno_core::OpState>) };

          let mut op_state = state.borrow_mut();
          let result = #name::<#type_params>(&mut op_state, a, b);

          op_state.tracker.track_sync(op_id);

          #ret
        }
      }
    })
  }
}
