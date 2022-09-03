use deno_core::v8::fast_api::FastApiCallbackOptions;
use deno_ops::op;

#[op(fast)]
fn op_fallback(options: Option<&mut FastApiCallbackOptions>) {
  if let Some(options) = options {
    options.fallback = true;
  }
}

fn main() {}
