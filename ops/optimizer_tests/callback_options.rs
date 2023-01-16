fn op_fallback(options: Option<&mut FastApiCallbackOptions>) {
  if let Some(options) = options {
    options.fallback = true;
  }
}
