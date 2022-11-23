use deno_core::v8::fast_api::FastApiCallbackOptions;
use deno_ops::op;

#[op(fast)]
fn op_fallback(options: Option<&mut FastApiCallbackOptions>) {
  if let Some(options) = options {
    options.fallback = true;
  }
}

#[op(fast)]
fn op_fast_str(string: &str) {
  println!("{}", string);
}

#[op(fast)]
fn op_fast_str_owned(string: &str) {
  println!("{}", string);
}

fn main() {}
