// Copyright 2018-2025 the Deno authors. MIT license.

use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::format_ident;
use quote::quote;
use syn::ImplItem;
use syn::ItemFn;
use syn::ItemImpl;
use syn::Token;
use syn::parse::ParseStream;
use syn::spanned::Spanned;

use crate::op2::MacroConfig;
use crate::op2::Op2Error;
use crate::op2::Op2ErrorKind;
use crate::op2::generate_op2;

use super::signature::is_attribute_special;

// Object wrap for Cppgc-backed objects
//
// This module generates the glue code declarations
// for `impl` blocks to create JS objects in Rust
// using the op2 infra.
//
// ```rust
// #[op]
// impl MyObject {
//    #[constructor] // <-- first attribute defines binding type
//    #[cppgc]       // <-- attributes for op2
//    fn new() -> MyObject {
//      MyObject::new()
//    }
//
//    #[static_method]
//    #[cppgc]
//    fn static_method() -> MyObject {
//      // ...
//    }
//
//    #[method]
//    #[smi]
//    fn method(&self) -> i32 {
//      // ...
//    }
//
//    #[getter]
//    fn x(&self) -> i32 {}
//
//    #[setter]
//    fn x(&self, x: i32) {}
// }
//
// The generated OpMethodDecl that can be passed to
// `deno_core::extension!` macro to register the object
//
// ```rust
// deno_core::extension!(
//   ...,
//   objects = [MyObject],
// )
// ```
//
// ```js
// import { MyObject } from "ext:core/ops";
// ```
//
// Supported bindings:
// - constructor
// - methods
// - static methods
// - getters
// - setters
//
pub(crate) fn generate_impl_ops(
  attr: TokenStream,
  item: ItemImpl,
) -> Result<TokenStream, Op2Error> {
  let args = syn::parse2::<Args>(attr)?;

  let mut tokens = TokenStream::new();

  let self_ty = &item.self_ty;
  let self_ty_ident = self_ty.to_token_stream().to_string();

  // State
  let mut constructor = None;
  let mut methods = Vec::new();
  let mut static_methods = Vec::new();

  for item in item.items {
    if let ImplItem::Fn(mut method) = item {
      let span = method.span();
      let (item_fn_attrs, attrs): (Vec<_>, Vec<_>) =
        method.attrs.into_iter().partition(is_attribute_special);

      /* Convert snake_case to camelCase */
      method.sig.ident = format_ident!(
        "{}",
        stringcase::camel_case(&method.sig.ident.to_string())
      );

      let mut func = ItemFn {
        attrs: item_fn_attrs,
        vis: method.vis,
        sig: method.sig,
        block: Box::new(method.block),
      };

      let mut config = MacroConfig::from_attributes(span, attrs)
        .map_err(|e| e.with_default_span(span))?;

      if args.is_base {
        config.use_cppgc_base = true;
      }

      if let Some(ref rename) = config.rename {
        if syn::parse_str::<syn::Ident>(rename).is_err() {
          // Keep the original function name if rename is a keyword
        } else {
          func.sig.ident = format_ident!("{}", rename);
        }
      }

      let ident = func.sig.ident.clone();
      if config.constructor {
        if constructor.is_some() {
          return Err(Op2Error::with_span(
            span,
            Op2ErrorKind::MultipleConstructors,
          ));
        }

        constructor = Some(ident);
      } else if config.static_member {
        static_methods.push(format_ident!("__static_{}", ident));
      } else {
        if config.setter {
          methods.push(format_ident!("__set_{}", ident));
        } else {
          methods.push(ident);
        }

        config.method = Some(format_ident!("{}", self_ty_ident));
      }

      config.self_name = Some(format_ident!("{}", self_ty_ident));

      let op =
        generate_op2(config, func).map_err(|e| e.with_default_span(span))?;
      tokens.extend(op);
    }
  }

  let constructor = if let Some(constructor) = constructor {
    quote! { Some(#self_ty::#constructor()) }
  } else {
    quote! { None }
  };

  let inherits_type_name = match &args.inherits_from {
    Some(ty) => quote! {
      inherits_type_name: || Some(std::any::type_name::<#ty>()),
    },
    None => quote! {
      inherits_type_name: || None,
    },
  };

  let res = quote! {
      impl #self_ty {
        pub const DECL: deno_core::_ops::OpMethodDecl = deno_core::_ops::OpMethodDecl {
          methods: &[
            #(
              #self_ty::#methods(),
            )*
          ],
          static_methods: &[
            #(
              #self_ty::#static_methods(),
            )*
          ],
          constructor: #constructor,
          name: ::deno_core::__op_name_fast!(#self_ty),
          type_name: || std::any::type_name::<#self_ty>(),
          #inherits_type_name
        };

        #tokens
      }
  };

  Ok(res)
}

struct Args {
  is_base: bool,
  inherits_from: Option<syn::Type>,
}

impl syn::parse::Parse for Args {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut is_base = false;
    let mut inherits_from = None;

    while !input.is_empty() {
      let lookahead = input.lookahead1();
      if lookahead.peek(super::kw::base) {
        input.parse::<super::kw::base>()?;
        is_base = true;
      } else if lookahead.peek(super::kw::inherit) {
        input.parse::<super::kw::inherit>()?;
        input.parse::<Token![=]>()?;
        inherits_from = Some(input.parse::<syn::Type>()?);
      } else {
        return Err(lookahead.error());
      }
      // consume optional comma between items
      let _ = input.parse::<Option<Token![,]>>();
    }

    Ok(Args {
      is_base,
      inherits_from,
    })
  }
}
