// Copyright 2018-2025 the Deno authors. MIT license.

use proc_macro2::Delimiter;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::Ident;
use syn::MacroDelimiter;
use syn::MetaList;
use syn::Path;
use syn::Token;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse2;

use crate::op2::Op2Error;
use crate::op2::Op2ErrorKind;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct MacroConfig {
  /// Generate a fastcall method (must be fastcall compatible).
  pub fast: bool,
  /// Do not generate a fastcall method (must be fastcall compatible).
  pub nofast: bool,
  /// Use other ops for the fast alternatives, rather than generating one for this op.
  pub fast_alternative: Option<String>,
  pub fake_async: bool,
  /// Marks a lazy async function (async must also be true).
  pub async_lazy: bool,
  /// Marks a deferred async function (async must also be true).
  pub async_deferred: bool,
  /// Marks an op as re-entrant (can safely call other ops).
  pub reentrant: bool,
  /// Marks an op as a method on a wrapped object.
  pub method: Option<Ident>,
  /// Same as method name but also used by static and constructor ops.
  pub self_name: Option<Ident>,
  /// Marks an op as a constructor
  pub constructor: bool,
  /// Marks an op as a static member
  pub static_member: bool,
  /// Marks an op with no side effects.
  pub no_side_effects: bool,
  /// Marks an op as a getter.
  pub getter: bool,
  /// Marks an op as a setter.
  pub setter: bool,
  /// Marks an op to have it collect stack trace of the call site in the OpState.
  pub stack_trace: bool,
  /// Total required number of arguments for the op.
  pub required: Option<u8>,
  /// Rename the op to the given name.
  pub rename: Option<String>,
  /// Symbol.for("op_name") for the op.
  pub symbol: bool,
  /// Use proto for cppgc object.
  pub use_cppgc_base: bool,
  /// Calls the fn with the promise_id of the async op.
  pub promise_id: bool,
  pub validate: Option<Path>,
}

impl MacroConfig {
  pub fn from_attributes(
    span: Span,
    attributes: Vec<syn::Attribute>,
  ) -> Result<Self, Op2Error> {
    let flags = attributes
      .into_iter()
      .filter(|attribute| {
        // Skip standard Rust attributes that aren't op2 config flags.
        // These end up here because object_wrap partitions attrs into
        // "op2 signature attrs" vs "everything else", and the latter
        // includes doc comments, #[allow], #[cfg], etc.
        !matches!(
          attribute
            .path()
            .get_ident()
            .map(|i| i.to_string())
            .as_deref(),
          Some("doc" | "allow" | "cfg")
        )
      })
      .map(|attribute| {
        let meta = attribute.meta;

        parse2::<CustomMeta>(meta.to_token_stream())
      })
      .collect::<Result<Vec<_>, _>>()?;
    Self::from_metas(span, flags)
  }

  pub fn from_metas(
    span: Span,
    flags: Vec<CustomMeta>,
  ) -> Result<Self, Op2Error> {
    let mut config = Self::default();
    let mut passed_flags = vec![];

    for meta in &flags {
      let flag = parse2::<Flags>(meta.to_token_stream())?;

      match &flag {
        Flags::Constructor => config.constructor = true,
        Flags::Getter => config.getter = true,
        Flags::Setter => config.setter = true,
        Flags::Fast(alternative) => {
          if let Some(path) = alternative {
            config.fast_alternative = Some(path.clone());
          } else {
            config.fast = true;
          }
        }
        Flags::NoFast => config.nofast = true,
        Flags::Async(async_mode) => match async_mode {
          AsyncMode::Fake => config.fake_async = true,
          AsyncMode::Lazy => config.async_lazy = true,
          AsyncMode::Deferred => config.async_deferred = true,
        },
        Flags::Reentrant => config.reentrant = true,
        Flags::NoSideEffects => config.no_side_effects = true,
        Flags::StackTrace => config.stack_trace = true,
        Flags::StaticMethod => config.static_member = true,
        Flags::Required(req) => config.required = Some(*req),
        Flags::Rename(rename) => config.rename = Some(rename.clone()),
        Flags::Symbol(symbol) => {
          config.rename = Some(symbol.clone());
          config.symbol = true;
        }
        Flags::PromiseId => config.promise_id = true,
        Flags::Validate(path) => {
          config.validate = Some(path.0.clone());
        }
      }

      passed_flags.push(flag);
    }

    // Ensure that the flags are sorted in alphabetical order for consistency and searchability
    if !passed_flags.is_sorted() {
      return Err(
        syn::Error::new(
          span,
          "The flags for this attribute were not sorted alphabetically.",
        )
        .into(),
      );
    }

    // Test for invalid attribute combinations
    if config.fast && config.nofast {
      return Err(Op2Error::with_span(
        span,
        Op2ErrorKind::InvalidAttributeCombination("fast", "nofast"),
      ));
    }
    if config.fast && config.fast_alternative.is_some() {
      return Err(Op2Error::with_span(
        span,
        Op2ErrorKind::InvalidAttributeCombination("fast", "fast(...)"),
      ));
    }
    if config.no_side_effects && config.reentrant {
      return Err(Op2Error::with_span(
        span,
        Op2ErrorKind::InvalidAttributeCombination(
          "no_side_effects",
          "reentrant",
        ),
      ));
    }

    Ok(config)
  }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
enum AsyncMode {
  Deferred,
  Fake,
  Lazy,
}

// keep in alphabetical order
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
enum Flags {
  Async(AsyncMode),
  Constructor,
  Fast(Option<String>),
  Getter,
  NoFast,
  NoSideEffects,
  Reentrant,
  Rename(String),
  Required(u8),
  Setter,
  StaticMethod,
  StackTrace,
  Symbol(String),
  PromiseId,
  Validate(PathOrd),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PathOrd(Path);

impl Ord for PathOrd {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self
      .0
      .segments
      .len()
      .cmp(&other.0.segments.len())
      .then_with(|| {
        self
          .0
          .to_token_stream()
          .to_string()
          .cmp(&other.0.to_token_stream().to_string())
      })
  }
}

impl PartialOrd for PathOrd {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Parse for Flags {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let lookahead = input.lookahead1();

    let flag = if lookahead.peek(kw::static_method) {
      input.parse::<kw::static_method>()?;
      Flags::StaticMethod
    } else if lookahead.peek(kw::constructor) {
      input.parse::<kw::constructor>()?;
      Flags::Constructor
    } else if lookahead.peek(kw::getter) {
      input.parse::<kw::getter>()?;
      Flags::Getter
    } else if lookahead.peek(kw::setter) {
      input.parse::<kw::setter>()?;
      Flags::Setter
    } else if lookahead.peek(kw::fast) {
      match input.parse::<CustomMeta>()?.as_meta_list() {
        Some(list) => {
          let ty = list.parse_args::<Type>()?;
          Flags::Fast(Some(ty.into_token_stream().to_string().replace(' ', "")))
        }
        _ => Flags::Fast(None),
      }
    } else if lookahead.peek(kw::nofast) {
      input.parse::<kw::nofast>()?;
      Flags::NoFast
    } else if lookahead.peek(Token![async]) || lookahead.peek(kw::async_method)
    {
      let list = input.parse::<CustomMeta>()?.require_list()?;
      let async_mode = list.parse_args_with(|input: ParseStream| {
        let lookahead = input.lookahead1();

        if lookahead.peek(kw::fake) {
          input.parse::<kw::fake>()?;
          Ok(AsyncMode::Fake)
        } else if lookahead.peek(kw::lazy) {
          input.parse::<kw::lazy>()?;
          Ok(AsyncMode::Lazy)
        } else if lookahead.peek(kw::deferred) {
          input.parse::<kw::deferred>()?;
          Ok(AsyncMode::Deferred)
        } else {
          Err(lookahead.error())
        }
      })?;

      Flags::Async(async_mode)
    } else if lookahead.peek(kw::reentrant) {
      input.parse::<kw::reentrant>()?;
      Flags::Reentrant
    } else if lookahead.peek(kw::no_side_effects) {
      input.parse::<kw::no_side_effects>()?;
      Flags::NoSideEffects
    } else if lookahead.peek(kw::stack_trace) {
      input.parse::<kw::stack_trace>()?;
      Flags::StackTrace
    } else if lookahead.peek(kw::required) {
      let meta = input.parse::<CustomMeta>()?;
      let list = meta.require_list()?;
      let lit = list.parse_args::<syn::LitInt>()?;
      Flags::Required(lit.base10_parse()?)
    } else if lookahead.peek(kw::rename) {
      let meta = input.parse::<CustomMeta>()?;
      let list = meta.require_list()?;
      let lit = list.parse_args::<syn::LitStr>()?;
      Flags::Rename(lit.value())
    } else if lookahead.peek(kw::symbol) {
      let meta = input.parse::<CustomMeta>()?;
      let list = meta.require_list()?;
      let lit = list.parse_args::<syn::LitStr>()?;
      Flags::Symbol(lit.value())
    } else if lookahead.peek(kw::promise_id) {
      input.parse::<kw::promise_id>()?;
      Flags::PromiseId
    } else if lookahead.peek(kw::validate) {
      let meta = input.parse::<CustomMeta>()?;
      let list = meta.require_list()?;
      let path = list.parse_args::<Path>()?;
      Flags::Validate(PathOrd(path))
    } else {
      return Err(lookahead.error());
    };

    Ok(flag)
  }
}

/// custom Meta-like struct to handle the `async` keyword.
#[derive(Debug)]
pub struct CustomMeta {
  ident: Ident,
  args: Option<(MacroDelimiter, TokenStream)>,
}

impl Parse for CustomMeta {
  fn parse(input: ParseStream) -> Result<Self, syn::Error> {
    let ident = if input.peek(Token![async]) {
      let token = input.parse::<Token![async]>()?;
      Ident::new("async", token.span)
    } else {
      input.parse()?
    };

    let args = if input.peek(syn::token::Paren) {
      let (delimiter, tokenstream) =
        input.step(|cursor| match cursor.token_tree() {
          Some((proc_macro2::TokenTree::Group(g), rest)) => {
            let span = g.delim_span();

            let delimiter = match g.delimiter() {
              Delimiter::Parenthesis => {
                MacroDelimiter::Paren(syn::token::Paren(span))
              }
              _ => {
                return Err(cursor.error("expected `(`"));
              }
            };

            Ok(((delimiter, g.stream()), rest))
          }
          _ => Err(cursor.error("expected delimiter")),
        })?;

      Some((delimiter, tokenstream))
    } else if input.peek(Token![=]) {
      return Err(input.error("expected `(`"));
    } else {
      None
    };

    Ok(CustomMeta { ident, args })
  }
}

impl CustomMeta {
  pub fn as_meta_list(&self) -> Option<MetaList> {
    self.args.clone().map(|(delimiter, tokens)| MetaList {
      path: syn::Path::from(self.ident.clone()),
      delimiter,
      tokens,
    })
  }

  pub fn require_list(&self) -> Result<MetaList, syn::Error> {
    self.as_meta_list().ok_or_else(|| {
      syn::Error::new(
        self.ident.span(),
        format!(
          "expected attribute arguments in parentheses: `{}(...)`",
          self.ident
        ),
      )
    })
  }
}

impl ToTokens for CustomMeta {
  fn to_tokens(&self, tokens: &mut TokenStream) {
    match self.as_meta_list() {
      Some(list) => {
        list.to_tokens(tokens);
      }
      _ => {
        self.ident.to_tokens(tokens);
      }
    }
  }
}

mod kw {
  use syn::custom_keyword;

  custom_keyword!(constructor);
  custom_keyword!(static_method);
  custom_keyword!(getter);
  custom_keyword!(setter);
  custom_keyword!(fast);
  custom_keyword!(nofast);
  custom_keyword!(async_method);
  custom_keyword!(reentrant);
  custom_keyword!(no_side_effects);
  custom_keyword!(stack_trace);
  custom_keyword!(required);
  custom_keyword!(rename);
  custom_keyword!(symbol);
  custom_keyword!(fake);
  custom_keyword!(lazy);
  custom_keyword!(deferred);
  custom_keyword!(promise_id);
  custom_keyword!(validate);
}

#[cfg(test)]
mod tests {
  use super::*;
  use syn::ItemFn;
  use syn::Meta;
  use syn::parse::Parser;
  use syn::punctuated::Punctuated;
  use syn::spanned::Spanned;

  fn test_parse(s: &str, expected: MacroConfig) {
    let item_fn = syn::parse_str::<ItemFn>(&format!("#[op2{s}] fn x() {{ }}"))
      .expect("Failed to parse function");
    let attr = item_fn.attrs.first().unwrap();
    match &attr.meta {
      Meta::List(list) => {
        let metas = Punctuated::<CustomMeta, Token![,]>::parse_terminated
          .parse2(list.tokens.clone())
          .expect("Failed to parse attribute")
          .into_iter()
          .collect::<Vec<_>>();

        let config = MacroConfig::from_metas(list.span(), metas)
          .expect("Failed to parse attribute");
        assert_eq!(expected, config);
      }
      _ => {
        match &attr.meta {
          Meta::Path(..) => {
            // Ignored
          }
          _ => {
            panic!("Not a list or path");
          }
        }
      }
    }
  }

  #[test]
  fn test_macro_parse() {
    test_parse("", MacroConfig::default());
    test_parse(
      "(promise_id)",
      MacroConfig {
        promise_id: true,
        ..Default::default()
      },
    );
    test_parse(
      "(async(lazy))",
      MacroConfig {
        async_lazy: true,
        ..Default::default()
      },
    );
    test_parse(
      "(fast(op_other1))",
      MacroConfig {
        fast_alternative: Some("op_other1".to_owned()),
        ..Default::default()
      },
    );
    test_parse(
      "(fast(op_generic::<T>))",
      MacroConfig {
        fast_alternative: Some("op_generic::<T>".to_owned()),
        ..Default::default()
      },
    );
  }
}
