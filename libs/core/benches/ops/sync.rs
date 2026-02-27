// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(deprecated)]
use bencher::*;
use deno_core::*;
use std::borrow::Cow;
use std::ffi::c_void;

deno_core::extension!(
  testing,
  ops = [
    op_void,
    op_void_nofast,
    op_void_no_side_effects,
    op_void_nofast_no_side_effects,
    op_void_metrics,
    op_void_nofast_metrics,
    op_u32,
    op_option_u32,
    op_string,
    op_string_onebyte,
    op_string_bytestring,
    op_string_bytestring_no_side_effects,
    op_string_option_u32,
    op_local,
    op_local_scope,
    op_local_nofast,
    op_scope,
    op_isolate_nofast,
    op_make_external,
    op_bigint,
    op_bigint_return,
    op_external,
    op_external_nofast,
    op_buffer,
    op_buffer_jsbuffer,
    op_buffer_nofast,
    op_arraybuffer,
  ],
  state = |state| {
    state.put(1234u32);
    state.put(10000u16);
  }
);

#[op2(fast)]
pub fn op_void() {}

#[op2(fast, no_side_effects)]
pub fn op_void_no_side_effects() {}

#[op2(nofast)]
pub fn op_void_nofast() {}

#[op2(nofast, no_side_effects)]
pub fn op_void_nofast_no_side_effects() {}

#[op2(fast)]
pub fn op_void_metrics() {}

#[op2(nofast)]
pub fn op_void_nofast_metrics() {}

#[op2(fast)]
pub fn op_u32() -> u32 {
  1
}

#[op2]
pub fn op_option_u32() -> Option<u32> {
  Some(1)
}

#[op2(fast)]
pub fn op_string(#[string] s: &str) -> u32 {
  s.len() as _
}

#[op2(fast)]
pub fn op_string_onebyte(#[string(onebyte)] s: Cow<[u8]>) -> u32 {
  s.len() as _
}

#[op2]
pub fn op_string_bytestring(#[serde] s: ByteString) -> u32 {
  s.len() as _
}

#[op2(no_side_effects)]
pub fn op_string_bytestring_no_side_effects(#[serde] s: ByteString) -> u32 {
  s.len() as _
}

#[op2]
pub fn op_string_option_u32(#[string] s: &str) -> Option<u32> {
  Some(s.len() as _)
}

#[op2(fast)]
pub fn op_local(_s: v8::Local<v8::String>) {}

#[op2(fast)]
pub fn op_local_scope<'s>(
  _scope: &mut v8::PinScope<'s, '_>,
  _s: v8::Local<'s, v8::String>,
) {
}

#[op2(nofast)]
pub fn op_local_nofast(_s: v8::Local<v8::String>) {}

#[op2(fast)]
pub fn op_scope(_scope: &mut v8::PinScope) {}

#[op2(nofast)]
pub fn op_isolate_nofast(_isolate: &mut v8::Isolate) {}

#[op2(fast)]
pub fn op_make_external() -> *const c_void {
  std::ptr::null()
}

#[op2(fast)]
pub fn op_bigint(#[bigint] _input: u64) {}

#[op2(fast)]
#[bigint]
pub fn op_bigint_return() -> u64 {
  0
}

#[op2(fast)]
pub fn op_external(_input: *const c_void) {}

#[op2(nofast)]
pub fn op_external_nofast(_input: *const c_void) {}

#[op2(fast)]
pub fn op_buffer(#[buffer] _buffer: &[u8]) {}

#[op2]
pub fn op_buffer_jsbuffer(#[buffer] _buffer: JsBuffer) {}

#[op2(nofast)]
pub fn op_buffer_nofast(#[buffer] _buffer: &[u8]) {}

#[op2(fast)]
pub fn op_arraybuffer(#[arraybuffer] _buffer: &[u8]) {}

fn bench_op(
  b: &mut Bencher,
  count: usize,
  op: &str,
  arg_count: usize,
  call: &str,
) {
  #[cfg(not(feature = "unsafe_runtime_options"))]
  unreachable!(
    "This benchmark must be run with --features=unsafe_runtime_options"
  );

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![testing::init()],
    // We need to feature gate this here to prevent IDE errors
    #[cfg(feature = "unsafe_runtime_options")]
    unsafe_expose_natives_and_gc: true,
    // Add metrics just for the metrics ops
    op_metrics_factory_fn: Some(Box::new(|_, _, op| {
      if op.name.ends_with("_metrics") {
        Some(std::rc::Rc::new(|_, _, _| {}))
      } else {
        None
      }
    })),
    ..Default::default()
  });
  let err_mapper = |err| {
    deno_error::JsErrorBox::generic(format!(
      "{op} test failed ({call}): {err:?}"
    ))
  };

  let args = (0..arg_count)
    .map(|n| format!("arg{n}"))
    .collect::<Vec<_>>()
    .join(", ");

  let mut harness = include_str!("sync_harness.js").to_owned();
  for (key, value) in [
    ("PERCENT", "%"),
    ("CALL", call),
    ("COUNT", &format!("{count}")),
    ("ARGS", &args.to_string()),
    ("OP", op),
    ("INIT", if op.contains("bigint") { "0n" } else { "0" }),
  ] {
    harness = harness.replace(&format!("__{key}__"), value);
  }

  // Prime the optimizer
  runtime
    .execute_script("", harness)
    .map_err(err_mapper)
    .unwrap();
  let bench = runtime.execute_script("", ascii_str!("bench")).unwrap();
  deno_core::scope!(scope, &mut runtime);
  #[allow(clippy::unnecessary_fallible_conversions)]
  let bench: v8::Local<v8::Function> =
    v8::Local::<v8::Value>::new(scope, bench)
      .try_into()
      .unwrap();
  b.iter(|| {
    let recv = v8::undefined(scope).into();
    bench.call(scope, recv, &[]);
  });
}

const BENCH_COUNT: usize = 1000;
const LARGE_BENCH_COUNT: usize = 5;

/// Tests the overhead of execute_script.
fn baseline(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_void", 0, "accum += __index__;");
}

/// A void function with no return value.
fn bench_op_void(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_void", 0, "op_void()");
}

/// A void function with no return value.
fn bench_op_void_2x(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_void", 0, "op_void(); op_void()");
}

/// A void function with no return value.
fn bench_op_void_nofast(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_void_nofast", 0, "op_void_nofast();");
}

fn bench_op_void_no_side_effects(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_void_no_side_effects",
    0,
    "op_void_no_side_effects();",
  );
}

fn bench_op_void_nofast_no_side_effects(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_void_nofast_no_side_effects",
    0,
    "op_void_nofast_no_side_effects();",
  );
}

/// A void function with no return value.
fn bench_op_void_metrics(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_void_metrics", 0, "op_void_metrics()");
}

/// A void function with no return value.
fn bench_op_void_nofast_metrics(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_void_nofast_metrics",
    0,
    "op_void_nofast_metrics()",
  );
}

/// A function with a numeric return value.
fn bench_op_u32(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_u32", 0, "accum += op_u32();");
}

/// A function with an optional return value (making it non-fast).
fn bench_op_option_u32(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_option_u32",
    0,
    "accum += op_option_u32();",
  );
}

/// A string function with a numeric return value.
fn bench_op_string(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string",
    1,
    "accum += op_string('this is a reasonably long string that we would like to get the length of!');",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_large_1000(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string",
    1,
    "accum += op_string(LARGE_STRING_1000);",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_large_1000000(b: &mut Bencher) {
  bench_op(
    b,
    LARGE_BENCH_COUNT,
    "op_string",
    1,
    "accum += op_string(LARGE_STRING_1000000);",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_onebyte(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string_onebyte",
    1,
    "accum += op_string_onebyte('this is a reasonably long string that we would like to get the length of!');",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_onebyte_large_1000(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string_onebyte",
    1,
    "accum += op_string_onebyte(LARGE_STRING_1000);",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_onebyte_large_1000000(b: &mut Bencher) {
  bench_op(
    b,
    LARGE_BENCH_COUNT,
    "op_string_onebyte",
    1,
    "accum += op_string_onebyte(LARGE_STRING_1000000);",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_bytestring(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string_bytestring",
    1,
    "accum += op_string_bytestring('this is a reasonably long string that we would like to get the length of!');",
  );
}

fn bench_op_string_bytestring_no_side_effects(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string_bytestring_no_side_effects",
    1,
    "accum += op_string_bytestring_no_side_effects('this is a reasonably long string that we would like to get the length of!');",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_large_utf8_1000(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string",
    1,
    "accum += op_string(LARGE_STRING_UTF8_1000);",
  );
}

/// A string function with a numeric return value.
fn bench_op_string_large_utf8_1000000(b: &mut Bencher) {
  bench_op(
    b,
    LARGE_BENCH_COUNT,
    "op_string",
    1,
    "accum += op_string(LARGE_STRING_UTF8_1000000);",
  );
}

/// A string function with an option numeric return value.
fn bench_op_string_option_u32(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_string_option_u32",
    1,
    "accum += op_string_option_u32('this is a reasonably long string that we would like to get the length of!');",
  );
}

/// A fast function that takes a v8::Local<String>
fn bench_op_v8_local(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_local",
    1,
    "op_local('this is a reasonably long string that we would like to get the length of!');",
  );
}

/// A function that takes a v8::Local<String>
fn bench_op_v8_local_scope(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_local_scope",
    1,
    "op_local_scope('this is a reasonably long string that we would like to get the length of!');",
  );
}

/// A function that takes a v8::Local<String>
fn bench_op_v8_local_nofast(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_local_nofast",
    1,
    "op_local_nofast('this is a reasonably long string that we would like to get the length of!');",
  );
}

fn bench_op_bigint(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_bigint", 1, "op_bigint(0n);");
}

fn bench_op_bigint_return(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_bigint_return",
    1,
    "accum += op_bigint_return();",
  );
}

/// A function that takes only a scope
fn bench_op_v8_scope(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_scope", 1, "op_scope();");
}

/// A function that takes only an isolate
fn bench_op_v8_isolate_nofast(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_isolate_nofast",
    1,
    "op_isolate_nofast();",
  );
}

fn bench_op_external(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_external", 1, "op_external(EXTERNAL)");
}

fn bench_op_external_nofast(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_external_nofast",
    1,
    "op_external_nofast(EXTERNAL)",
  );
}

fn bench_op_buffer(b: &mut Bencher) {
  bench_op(b, BENCH_COUNT, "op_buffer", 1, "op_buffer(BUFFER)");
}

fn bench_op_buffer_jsbuffer(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_buffer_jsbuffer",
    1,
    "op_buffer_jsbuffer(BUFFER)",
  );
}

fn bench_op_buffer_nofast(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_buffer_nofast",
    1,
    "op_buffer_nofast(BUFFER)",
  );
}

fn bench_op_arraybuffer(b: &mut Bencher) {
  bench_op(
    b,
    BENCH_COUNT,
    "op_arraybuffer",
    1,
    "op_arraybuffer(ARRAYBUFFER)",
  );
}

benchmark_group!(
  benches,
  baseline,
  bench_op_void,
  bench_op_void_2x,
  bench_op_void_nofast,
  bench_op_void_no_side_effects,
  bench_op_void_nofast_no_side_effects,
  bench_op_void_metrics,
  bench_op_void_nofast_metrics,
  bench_op_u32,
  bench_op_option_u32,
  bench_op_string_bytestring,
  bench_op_string_bytestring_no_side_effects,
  bench_op_string,
  bench_op_string_large_1000,
  bench_op_string_large_1000000,
  bench_op_string_onebyte,
  bench_op_string_onebyte_large_1000,
  bench_op_string_onebyte_large_1000000,
  bench_op_string_large_utf8_1000,
  bench_op_string_large_utf8_1000000,
  bench_op_string_option_u32,
  bench_op_v8_local,
  bench_op_v8_local_scope,
  bench_op_v8_local_nofast,
  bench_op_bigint,
  bench_op_bigint_return,
  bench_op_v8_scope,
  bench_op_v8_isolate_nofast,
  bench_op_external,
  bench_op_external_nofast,
  bench_op_buffer,
  bench_op_buffer_jsbuffer,
  bench_op_buffer_nofast,
  bench_op_arraybuffer,
);

benchmark_main!(benches);
