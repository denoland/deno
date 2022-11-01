use attrs::Attributes;
use proc_macro::TokenStream;
use syn::{
    parse,
    ItemFn,
    parse_macro_input,
}
#[cfg(test)]
mod tests;

mod attrs;

#[proc_macro_attribute]
pub fn op(attr: TokenStream, item: TokenStream) -> TokenStream {
  let Attributes { .. } = parse_macro_input!(attr as Attributes);
  let func = parse::<ItemFn>(item).expect("expected a function");

  let mut tts = q!({});

  tts.dump().into()
}

/// Shortcut for `Quote::new_call_site().quote_with(smart_quote!( $tokens ))`
#[macro_export]
macro_rules! q {
    ( $($tokens:tt)* ) => {{
        $crate::Quote::new_call_site().quote_with($crate::smart_quote!( $($tokens)* ))
    }};
}

/// Blocks emitted by the macro, in order.
///
/// ```no_run,rust
/// #[op]
/// fn foo() {}
///
/// struct foo;
///
/// impl foo {
///   // orig function preserved.
///   fn call() {}
///
///   ...
/// }
///
/// extern "C" fn foo(info: *const FunctionCallbackInfo) {}
/// 
/// struct fast_foo;
/// impl fast_api::FastFunction for fast_foo {
///    ...
/// }
/// ```
struct Blocks<T> {
    ty: T,
    slow_fn: T,
    fast_fn: Option<T>,
}