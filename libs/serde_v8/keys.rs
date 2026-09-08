// Copyright 2018-2026 the Deno authors. MIT license.

// creates an optimized v8::String for a struct field
// TODO: experiment with external strings
// TODO: evaluate if own KeyCache is better than v8's dedupe
pub fn v8_struct_key<'s, 'i>(
  scope: &v8::PinScope<'s, 'i>,
  field: &'static str,
) -> v8::Local<'s, v8::String> {
  // Internalized v8 strings are significantly faster than "normal" v8 strings
  // since v8 deduplicates re-used strings minimizing new allocations
  // see: https://github.com/v8/v8/blob/14ac92e02cc3db38131a57e75e2392529f405f2f/include/v8.h#L3165-L3171
  v8::String::new_from_utf8(
    scope,
    field.as_ref(),
    v8::NewStringType::Internalized,
  )
  .unwrap()

  // TODO: consider external strings later
  // right now non-deduped external strings (without KeyCache)
  // are slower than the deduped internalized strings by ~2.5x
  // since they're a new string in v8's eyes and needs to be hashed, etc...
  // v8::String::new_external_onebyte_static(scope, field).unwrap()
}
