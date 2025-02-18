// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file

// Run using cargo test or `--v8-flags=--allow-natives-syntax`

import {
  assertThrows,
  assert,
  assertNotEquals,
  assertInstanceOf,
  assertEquals,
  assertFalse,
} from "@std/assert";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const resourcesPre = Deno[Deno.internal].core.resources();

// dlopen shouldn't panic
assertThrows(() => {
  Deno.dlopen("cli/src/main.rs", {});
});

assertThrows(
  () => {
    Deno.dlopen(libPath, {
      non_existent_symbol: {
        parameters: [],
        result: "void",
      },
    });
  },
  Error,
  "Failed to register symbol non_existent_symbol",
);

assertThrows(() => {
  Deno.dlopen(libPath, {
    print_something: {
      parameters: [],
      result: { struct: [] }
    },
  }),
  TypeError,
  "Struct must have at least one field"
});

assertThrows(() => {
  Deno.dlopen(libPath, {
    print_something: {
      parameters: [ { struct: [] } ],
      result: "void",
    },
  }),
  TypeError,
  "Struct must have at least one field"
});

const Empty = { struct: [] }
assertThrows(() => {
  Deno.dlopen(libPath, {
    print_something: {
      parameters: [ { struct: [Empty] } ],
      result: "void",
    },
  }),
  TypeError,
  "Struct must have at least one field"
});

const Point = ["f64", "f64"];
const Size = ["f64", "f64"];
const Rect = ["f64", "f64", "f64", "f64"];
const RectNested = [{ struct: Point }, { struct: Size }];
const RectNestedCached = [{ struct: Size }, { struct: Size }];
const Mixed = ["u8", "f32", { struct: Rect }, "usize", { struct: ["u32", "u32"] }];

const dylib = Deno.dlopen(libPath, {
  "printSomething": {
    name: "print_something",
    parameters: [],
    result: "void",
  },
  "print_buffer": { parameters: ["buffer", "usize"], result: "void" },
  "print_pointer": { name: "print_buffer", parameters: ["pointer", "usize"], result: "void" },
  "print_buffer2": {
    parameters: ["buffer", "usize", "buffer", "usize"],
    result: "void",
  },
  "return_buffer": { parameters: [], result: "buffer" },
  "is_null_ptr": { parameters: ["pointer"], result: "bool" },
  "is_null_buf": { name: "is_null_ptr", parameters: ["buffer"], result: "bool" },
  "add_u32": { parameters: ["u32", "u32"], result: "u32" },
  "add_i32": { parameters: ["i32", "i32"], result: "i32" },
  "add_u64": { parameters: ["u64", "u64"], result: "u64" },
  "add_i64": { parameters: ["i64", "i64"], result: "i64" },
  "add_usize": { parameters: ["usize", "usize"], result: "usize" },
  "add_usize_fast": { parameters: ["usize", "usize"], result: "u32" },
  "add_isize": { parameters: ["isize", "isize"], result: "isize" },
  "add_f32": { parameters: ["f32", "f32"], result: "f32" },
  "add_f64": { parameters: ["f64", "f64"], result: "f64" },
  "and": { parameters: ["bool", "bool"], result: "bool" },
  "add_u32_nonblocking": {
    name: "add_u32",
    parameters: ["u32", "u32"],
    result: "u32",
    nonblocking: true,
  },
  "add_i32_nonblocking": {
    name: "add_i32",
    parameters: ["i32", "i32"],
    result: "i32",
    nonblocking: true,
  },
  "add_u64_nonblocking": {
    name: "add_u64",
    parameters: ["u64", "u64"],
    result: "u64",
    nonblocking: true,
  },
  "add_i64_nonblocking": {
    name: "add_i64",
    parameters: ["i64", "i64"],
    result: "i64",
    nonblocking: true,
  },
  "add_usize_nonblocking": {
    name: "add_usize",
    parameters: ["usize", "usize"],
    result: "usize",
    nonblocking: true,
  },
  "add_isize_nonblocking": {
    name: "add_isize",
    parameters: ["isize", "isize"],
    result: "isize",
    nonblocking: true,
  },
  "add_f32_nonblocking": {
    name: "add_f32",
    parameters: ["f32", "f32"],
    result: "f32",
    nonblocking: true,
  },
  "add_f64_nonblocking": {
    name: "add_f64",
    parameters: ["f64", "f64"],
    result: "f64",
    nonblocking: true,
  },
  "fill_buffer": { parameters: ["u8", "buffer", "usize"], result: "void" },
  "sleep_nonblocking": {
    name: "sleep_blocking",
    parameters: ["u64"],
    result: "void",
    nonblocking: true,
  },
  "sleep_blocking": { parameters: ["u64"], result: "void" },
  "nonblocking_buffer": {
    parameters: ["buffer", "usize"],
    result: "void",
    nonblocking: true,
  },
  "get_add_u32_ptr": {
    parameters: [],
    result: "pointer",
  },
  "get_sleep_blocking_ptr": {
    parameters: [],
    result: "pointer",
  },
  // Callback function
  call_fn_ptr: {
    parameters: ["function"],
    result: "void",
  },
  call_fn_ptr_thread_safe: {
    name: "call_fn_ptr",
    parameters: ["function"],
    result: "void",
    nonblocking: true,
  },
  call_fn_ptr_many_parameters: {
    parameters: ["function"],
    result: "void",
  },
  call_fn_ptr_return_u8: {
    parameters: ["function"],
    result: "void",
  },
  call_fn_ptr_return_u8_thread_safe: {
    name: "call_fn_ptr_return_u8",
    parameters: ["function"],
    result: "void",
  },
  call_fn_ptr_return_buffer: {
    parameters: ["function"],
    result: "void",
  },
  store_function: {
    parameters: ["function"],
    result: "void",
  },
  store_function_2: {
    parameters: ["function"],
    result: "void",
  },
  call_stored_function: {
    parameters: [],
    result: "void",
  },
  call_stored_function_2: {
    parameters: ["u8"],
    result: "void",
  },
  log_many_parameters: {
    parameters: ["u8", "u16", "u32", "u64", "f64", "f32", "i64", "i32", "i16", "i8", "isize", "usize", "f64", "f32", "f64", "f32", "f64", "f32", "f64"],
    result: "void",
  },
  cast_u8_u32: {
    parameters: ["u8"],
    result: "u32",
  },
  cast_u32_u8: {
    parameters: ["u32"],
    result: "u8",
  },
  add_many_u16: {
    parameters: ["u16", "u16", "u16", "u16", "u16", "u16", "u16", "u16", "u16", "u16", "u16", "u16", "u16"],
    result: "u16",
  },
  // Statics
  "static_u32": {
    type: "u32",
  },
  "static_i64": {
    type: "i64",
  },
  "static_ptr": {
    type: "pointer",
  },
  /**
   * Invalid UTF-8 characters, buffer of length 14
   */
  "static_char": {
    type: "pointer",
    optional: true,
  },
  "hash": { parameters: ["buffer", "u32"], result: "u32" },
  make_rect: {
    parameters: ["f64", "f64", "f64", "f64"],
    result: { struct: Rect },
  },
  make_rect_async: {
    name: "make_rect",
    nonblocking: true,
    parameters: ["f64", "f64", "f64", "f64"],
    result: { struct: RectNested },
  },
  print_rect: {
    parameters: [{ struct: RectNestedCached }],
    result: "void",
  },
  print_rect_async: {
    name: "print_rect",
    nonblocking: true,
    parameters: [{ struct: Rect }],
    result: "void",
  },
  create_mixed: {
    parameters: ["u8", "f32", { struct: Rect }, "pointer", "buffer"],
    result: { struct: Mixed }
  },
  print_mixed: {
    parameters: [{ struct: Mixed }],
    result: "void",
    optional: true,
  },
  non_existent_symbol: {
    parameters: [],
    result: "void",
    optional: true,
  },
  non_existent_nonblocking_symbol: {
    parameters: [],
    result: "void",
    nonblocking: true,
    optional: true,
  },
  non_existent_static: {
    type: "u32",
    optional: true,
  },
});
const { symbols } = dylib;

symbols.printSomething();
const buffer = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
const buffer2 = new Uint8Array([9, 10]);
dylib.symbols.print_buffer(buffer, buffer.length);
// Test subarrays
const subarray = buffer.subarray(3);
dylib.symbols.print_buffer(subarray, subarray.length - 2);
dylib.symbols.print_buffer2(buffer, buffer.length, buffer2, buffer2.length);

const { return_buffer } = symbols;
function returnBuffer() { return return_buffer(); };

%PrepareFunctionForOptimization(returnBuffer);
returnBuffer();
%OptimizeFunctionOnNextCall(returnBuffer);
const ptr0 = returnBuffer();
assertIsOptimized(returnBuffer);

dylib.symbols.print_pointer(ptr0, 8);
const ptrView = new Deno.UnsafePointerView(ptr0);
const into = new Uint8Array(6);
const into2 = new Uint8Array(3);
const into2ptr = Deno.UnsafePointer.of(into2);
const into2ptrView = new Deno.UnsafePointerView(into2ptr);
const into3 = new Uint8Array(3);
const into4 = new Uint16Array(3);
ptrView.copyInto(into4);
ptrView.copyInto(into);
console.log([...into]);
ptrView.copyInto(into2, 3);
console.log([...into2]);
into2ptrView.copyInto(into3);
console.log([...into3]);
const string = new Uint8Array([
  ...new TextEncoder().encode("Hello from pointer!"),
  0,
]);
const stringPtr = Deno.UnsafePointer.of(string);
const stringPtrview = new Deno.UnsafePointerView(stringPtr);
console.log(stringPtrview.getCString());
console.log(stringPtrview.getCString(11));
console.log("false", dylib.symbols.is_null_ptr(ptr0));
console.log("true", dylib.symbols.is_null_ptr(null));
console.log("false", dylib.symbols.is_null_ptr(Deno.UnsafePointer.of(into)));
const emptyBuffer = new Uint8Array(0);
console.log("true", dylib.symbols.is_null_ptr(Deno.UnsafePointer.of(emptyBuffer)));
const emptySlice = into.subarray(6);
console.log("false", dylib.symbols.is_null_ptr(Deno.UnsafePointer.of(emptySlice)));

const { is_null_buf } = symbols;
function isNullBuffer(buffer) { return is_null_buf(buffer); };
function isNullBufferDeopt(buffer) { return is_null_buf(buffer); };
%PrepareFunctionForOptimization(isNullBuffer);
isNullBuffer(emptyBuffer);
%NeverOptimizeFunction(isNullBufferDeopt);
%OptimizeFunctionOnNextCall(isNullBuffer);
isNullBuffer(emptyBuffer);
assertIsOptimized(isNullBuffer);

// ==== ZERO LENGTH BUFFER TESTS ====
assertEquals(isNullBuffer(emptyBuffer), true, "isNullBuffer(emptyBuffer) !== true");
assertEquals(isNullBufferDeopt(emptyBuffer), true, "isNullBufferDeopt(emptyBuffer) !== true");
assertEquals(isNullBuffer(emptySlice), false, "isNullBuffer(emptySlice) !== false");
assertEquals(isNullBufferDeopt(emptySlice), false, "isNullBufferDeopt(emptySlice) !== false");
assertEquals(isNullBuffer(new Uint8Array()), true, "isNullBuffer(new Uint8Array()) !== false");
assertEquals(isNullBufferDeopt(new Uint8Array()), true, "isNullBufferDeopt(new Uint8Array()) !== true");

// Externally backed ArrayBuffer has a non-null data pointer, even though its length is zero.
const externalZeroBuffer = new Uint8Array(Deno.UnsafePointerView.getArrayBuffer(ptr0, 0));
// V8 Fast calls used to get null pointers for all zero-sized buffers no matter their external backing.
assertEquals(isNullBuffer(externalZeroBuffer), false, "isNullBuffer(externalZeroBuffer) !== false");
// V8's `Local<ArrayBuffer>->Data()` method also used to similarly return null pointers for all
// zero-sized buffers which would not match what `Local<ArrayBuffer>->GetBackingStore()->Data()`
// API returned. These issues have been fixed in https://bugs.chromium.org/p/v8/issues/detail?id=13488.
assertEquals(isNullBufferDeopt(externalZeroBuffer), false, "isNullBufferDeopt(externalZeroBuffer) !== false");

// The same pointer with a non-zero byte length for the buffer will return non-null pointers in
// both Fast call and V8 API calls.
const externalOneBuffer = new Uint8Array(Deno.UnsafePointerView.getArrayBuffer(ptr0, 1));
assertEquals(isNullBuffer(externalOneBuffer), false, "isNullBuffer(externalOneBuffer) !== false");
assertEquals(isNullBufferDeopt(externalOneBuffer), false, "isNullBufferDeopt(externalOneBuffer) !== false");

// UnsafePointer.of uses an exact-pointer fallback for zero-length buffers and slices to ensure that it always gets
// the underlying pointer right.
assertNotEquals(Deno.UnsafePointer.of(externalZeroBuffer), null, "Deno.UnsafePointer.of(externalZeroBuffer) === null");
assertNotEquals(Deno.UnsafePointer.of(externalOneBuffer), null, "Deno.UnsafePointer.of(externalOneBuffer) === null");

const addU32Ptr = dylib.symbols.get_add_u32_ptr();
const addU32 = new Deno.UnsafeFnPointer(addU32Ptr, {
  parameters: ["u32", "u32"],
  result: "u32",
});
console.log(addU32.call(123, 456));

const sleepBlockingPtr = dylib.symbols.get_sleep_blocking_ptr();
const sleepNonBlocking = new Deno.UnsafeFnPointer(sleepBlockingPtr, {
  nonblocking: true,
  parameters: ["u64"],
  result: "void",
});
const before = performance.now();
await sleepNonBlocking.call(100);
console.log(performance.now() - before >= 100);

const { add_u32, add_usize_fast } = symbols;
function addU32Fast(a, b) {
  return add_u32(a, b);
};
testOptimized(addU32Fast, () => addU32Fast(123, 456));

function addU64Fast(a, b) { return add_usize_fast(a, b); };
testOptimized(addU64Fast, () => addU64Fast(2n, 3n));

console.log(dylib.symbols.add_i32(123, 456));
console.log(dylib.symbols.add_u64(0xffffffffn, 0xffffffffn));
console.log(dylib.symbols.add_i64(-0xffffffffn, -0xffffffffn));
console.log(dylib.symbols.add_usize(0xffffffffn, 0xffffffffn));
console.log(dylib.symbols.add_isize(-0xffffffffn, -0xffffffffn));
console.log(dylib.symbols.add_u64(Number.MAX_SAFE_INTEGER, 1));
console.log(dylib.symbols.add_i64(Number.MAX_SAFE_INTEGER, 1));
console.log(dylib.symbols.add_i64(Number.MIN_SAFE_INTEGER, -1));
console.log(dylib.symbols.add_usize(Number.MAX_SAFE_INTEGER, 1));
console.log(dylib.symbols.add_isize(Number.MAX_SAFE_INTEGER, 1));
console.log(dylib.symbols.add_isize(Number.MIN_SAFE_INTEGER, -1));
console.log(dylib.symbols.add_f32(123.123, 456.789));
console.log(dylib.symbols.add_f64(123.123, 456.789));
console.log(dylib.symbols.and(true, true));
console.log(dylib.symbols.and(true, false));

function addF32Fast(a, b) {
  return dylib.symbols.add_f32(a, b);
};
testOptimized(addF32Fast, () => addF32Fast(123.123, 456.789));

function addF64Fast(a, b) {
  return dylib.symbols.add_f64(a, b);
};
testOptimized(addF64Fast, () => addF64Fast(123.123, 456.789));

// Test adders as nonblocking calls
console.log(await dylib.symbols.add_i32_nonblocking(123, 456));
console.log(await dylib.symbols.add_u64_nonblocking(0xffffffffn, 0xffffffffn));
console.log(
  await dylib.symbols.add_i64_nonblocking(-0xffffffffn, -0xffffffffn),
);
console.log(
  await dylib.symbols.add_usize_nonblocking(0xffffffffn, 0xffffffffn),
);
console.log(
  await dylib.symbols.add_isize_nonblocking(-0xffffffffn, -0xffffffffn),
);
console.log(await dylib.symbols.add_u64_nonblocking(Number.MAX_SAFE_INTEGER, 1));
console.log(await dylib.symbols.add_i64_nonblocking(Number.MAX_SAFE_INTEGER, 1));
console.log(await dylib.symbols.add_i64_nonblocking(Number.MIN_SAFE_INTEGER, -1));
console.log(await dylib.symbols.add_usize_nonblocking(Number.MAX_SAFE_INTEGER, 1));
console.log(await dylib.symbols.add_isize_nonblocking(Number.MAX_SAFE_INTEGER, 1));
console.log(await dylib.symbols.add_isize_nonblocking(Number.MIN_SAFE_INTEGER, -1));
console.log(await dylib.symbols.add_f32_nonblocking(123.123, 456.789));
console.log(await dylib.symbols.add_f64_nonblocking(123.123, 456.789));

// test mutating sync calls

function test_fill_buffer(fillValue, arr) {
  let buf = new Uint8Array(arr);
  dylib.symbols.fill_buffer(fillValue, buf, buf.length);
  for (let i = 0; i < buf.length; i++) {
    if (buf[i] !== fillValue) {
      throw new Error(`Found '${buf[i]}' in buffer, expected '${fillValue}'.`);
    }
  }
}

test_fill_buffer(0, [2, 3, 4]);
test_fill_buffer(5, [2, 7, 3, 2, 1]);

const deferred = Promise.withResolvers();
const buffer3 = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
dylib.symbols.nonblocking_buffer(buffer3, buffer3.length).then(() => {
  deferred.resolve();
});
await deferred.promise;

let start = performance.now();
dylib.symbols.sleep_blocking(100);
assert(performance.now() - start >= 100);

start = performance.now();
const promise_2 = dylib.symbols.sleep_nonblocking(100).then(() => {
  console.log("After");
  assert(performance.now() - start >= 100);
});
console.log("Before");
assert(performance.now() - start < 100);

// Await to make sure `sleep_nonblocking` calls and logs before we proceed
await promise_2;

// Test calls with callback parameters
const logCallback = new Deno.UnsafeCallback(
  { parameters: [], result: "void" },
  () => console.log("logCallback"),
);
const logManyParametersCallback = new Deno.UnsafeCallback({
  parameters: [
    "u8",
    "i8",
    "u16",
    "i16",
    "u32",
    "i32",
    "u64",
    "i64",
    "f32",
    "f64",
    "pointer",
  ],
  result: "void",
}, (u8, i8, u16, i16, u32, i32, u64, i64, f32, f64, pointer) => {
  const view = new Deno.UnsafePointerView(pointer);
  const copy_buffer = new Uint8Array(8);
  view.copyInto(copy_buffer);
  console.log(u8, i8, u16, i16, u32, i32, u64, i64, f32, f64, ...copy_buffer);
});
const returnU8Callback = new Deno.UnsafeCallback(
  { parameters: [], result: "u8" },
  () => 8,
);
const returnBufferCallback = new Deno.UnsafeCallback({
  parameters: [],
  result: "buffer",
}, () => {
  return buffer;
});
const add10Callback = new Deno.UnsafeCallback({
  parameters: ["u8"],
  result: "u8",
}, (value) => value + 10);
const throwCallback = new Deno.UnsafeCallback({
  parameters: [],
  result: "void",
}, () => {
  throw new TypeError("hi");
});

assertThrows(
  () => {
    dylib.symbols.call_fn_ptr(throwCallback.pointer);
  },
  TypeError,
  "hi",
);

const { call_stored_function } = dylib.symbols;

dylib.symbols.call_fn_ptr(logCallback.pointer);
dylib.symbols.call_fn_ptr_many_parameters(logManyParametersCallback.pointer);
dylib.symbols.call_fn_ptr_return_u8(returnU8Callback.pointer);
dylib.symbols.call_fn_ptr_return_buffer(returnBufferCallback.pointer);
dylib.symbols.store_function(logCallback.pointer);
call_stored_function();
dylib.symbols.store_function_2(add10Callback.pointer);
dylib.symbols.call_stored_function_2(20);

function logManyParametersFast(a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s) {
  return symbols.log_many_parameters(a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s);
};
testOptimized(
  logManyParametersFast,
  () => logManyParametersFast(
    255, 65535, 4294967295, 4294967296n, 123.456, 789.876, -1n, -2, -3, -4, -1000n, 1000n,
    12345.678910, 12345.678910, 12345.678910, 12345.678910, 12345.678910, 12345.678910, 12345.678910
  )
);

// Some ABIs rely on the convention to zero/sign-extend arguments by the caller to optimize the callee function.
// If the trampoline did not zero/sign-extend arguments, this would return 256 instead of the expected 0 (in optimized builds)
function castU8U32Fast(x) { return symbols.cast_u8_u32(x); };
testOptimized(castU8U32Fast, () => castU8U32Fast(256));

// Some ABIs rely on the convention to expect garbage in the bits beyond the size of the return value to optimize the callee function.
// If the trampoline did not zero/sign-extend the return value, this would return 256 instead of the expected 0 (in optimized builds)
function castU32U8Fast(x) { return symbols.cast_u32_u8(x); };
testOptimized(castU32U8Fast, () => castU32U8Fast(256));

// Generally the trampoline tail-calls into the FFI function, but in certain cases (e.g. when returning 8 or 16 bit integers)
// the tail call is not possible and a new stack frame must be created. We need enough parameters to have some on the stack
function addManyU16Fast(a, b, c, d, e, f, g, h, i, j, k, l, m) {
  return symbols.add_many_u16(a, b, c, d, e, f, g, h, i, j, k, l, m);
};
// N.B. V8 does not currently follow Aarch64 Apple's calling convention.
// The current implementation of the JIT trampoline follows the V8 incorrect calling convention. This test covers the use-case
// and is expected to fail once Deno uses a V8 version with the bug fixed.
// The V8 bug is being tracked in https://bugs.chromium.org/p/v8/issues/detail?id=13171
testOptimized(addManyU16Fast, () => addManyU16Fast(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12));


const nestedCallback = new Deno.UnsafeCallback(
  { parameters: [], result: "void" },
  () => {
    dylib.symbols.call_stored_function_2(10);
  },
);
dylib.symbols.store_function(nestedCallback.pointer);

dylib.symbols.store_function(null);
dylib.symbols.store_function_2(null);

let counter = 0;
const addToFooCallback = new Deno.UnsafeCallback({
  parameters: [],
  result: "void",
}, () => counter++);

// Test thread safe callbacks
assertEquals(counter, 0);
addToFooCallback.ref();
await dylib.symbols.call_fn_ptr_thread_safe(addToFooCallback.pointer);
addToFooCallback.unref();
logCallback.ref();
await dylib.symbols.call_fn_ptr_thread_safe(logCallback.pointer);
logCallback.unref();
assertEquals(counter, 1);
returnU8Callback.ref();
await dylib.symbols.call_fn_ptr_return_u8_thread_safe(returnU8Callback.pointer);
// Purposefully do not unref returnU8Callback: Instead use it to test close() unrefing.

// Test statics
assertEquals(dylib.symbols.static_u32, 42);
assertEquals(dylib.symbols.static_i64, -1242464576485n);
assert(
  typeof dylib.symbols.static_ptr === "object"
);
assertEquals(
  Object.keys(dylib.symbols.static_ptr).length, 0
);
const view = new Deno.UnsafePointerView(dylib.symbols.static_ptr);
assertEquals(view.getUint32(), 42);

// Test struct returning
const rect_sync = dylib.symbols.make_rect(10, 20, 100, 200);
assertInstanceOf(rect_sync, Uint8Array);
assertEquals(rect_sync.length, 4 * 8);
assertEquals(Array.from(new Float64Array(rect_sync.buffer)), [10, 20, 100, 200]);
// Test struct passing
dylib.symbols.print_rect(rect_sync);
// Test struct passing asynchronously
await dylib.symbols.print_rect_async(rect_sync);
dylib.symbols.print_rect(new Float64Array([20, 20, 100, 200]));
// Test struct returning asynchronously
const rect_async = await dylib.symbols.make_rect_async(10, 20, 100, 200);
assertInstanceOf(rect_async, Uint8Array);
assertEquals(rect_async.length, 4 * 8);
assertEquals(Array.from(new Float64Array(rect_async.buffer)), [10, 20, 100, 200]);

// Test complex, mixed struct returning and passing
const mixedStruct = dylib.symbols.create_mixed(3, 12.515000343322754, rect_async, Deno.UnsafePointer.create(12456789), new Uint32Array([8, 32]));
assertEquals(mixedStruct.length, 56);
assertEquals(Array.from(mixedStruct.subarray(0, 4)), [3, 0, 0, 0]);
assertEquals(new Float32Array(mixedStruct.buffer, 4, 1)[0], 12.515000343322754);
assertEquals(new Float64Array(mixedStruct.buffer, 8, 4), new Float64Array(rect_async.buffer));
assertEquals(new BigUint64Array(mixedStruct.buffer, 40, 1)[0], 12456789n);
assertEquals(new Uint32Array(mixedStruct.buffer, 48, 2), new Uint32Array([8, 32]));
dylib.symbols.print_mixed(mixedStruct);

const cb = new Deno.UnsafeCallback({
  parameters: [{ struct: Rect }],
  result: { struct: Rect },
}, (innerRect) => {
  innerRect = new Float64Array(innerRect.buffer);
  return new Float64Array([innerRect[0] + 10, innerRect[1] + 10, innerRect[2] + 10, innerRect[3] + 10]);
});

const cbFfi = new Deno.UnsafeFnPointer(cb.pointer, cb.definition);
const cbResult = new Float64Array(cbFfi.call(rect_async).buffer);
assertEquals(Array.from(cbResult), [20, 30, 110, 210]);

cb.close();

const arrayBuffer = view.getArrayBuffer(4);
const uint32Array = new Uint32Array(arrayBuffer);
assertEquals(arrayBuffer.byteLength, 4);
assertEquals(uint32Array.length, 1);
assertEquals(uint32Array[0], 42);
uint32Array[0] = 55; // MUTATES!
assertEquals(uint32Array[0], 55);
assertEquals(view.getUint32(), 55);


{
  // Test UnsafePointer APIs
  assertEquals(Deno.UnsafePointer.create(0), null);
  const createdPointer = Deno.UnsafePointer.create(1);
  assertNotEquals(createdPointer, null);
  assertEquals(typeof createdPointer, "object");
  assertEquals(Deno.UnsafePointer.value(null), 0n);
  assertEquals(Deno.UnsafePointer.value(createdPointer), 1n);
  assert(Deno.UnsafePointer.equals(null, null));
  assertFalse(Deno.UnsafePointer.equals(null, createdPointer));
  assertFalse(Deno.UnsafePointer.equals(Deno.UnsafePointer.create(2), createdPointer));
  // Do not allow offsetting from null, `create` function should be used instead.
  assertThrows(() => Deno.UnsafePointer.offset(null, 5));
  const offsetPointer = Deno.UnsafePointer.offset(createdPointer, 5);
  assertEquals(Deno.UnsafePointer.value(offsetPointer), 6n);
  const zeroPointer = Deno.UnsafePointer.offset(offsetPointer, -6);
  assertEquals(Deno.UnsafePointer.value(zeroPointer), 0n);
  assertEquals(zeroPointer, null);
}

// Test non-UTF-8 characters

const charView = new Deno.UnsafePointerView(dylib.symbols.static_char);

const charArrayBuffer = charView.getArrayBuffer(14);
const uint8Array = new Uint8Array(charArrayBuffer);
assertEquals([...uint8Array], [
  0xC0, 0xC1, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
  0x00
]);

// Check that `getCString` works equally to `TextDecoder`
assertEquals(charView.getCString(), new TextDecoder().decode(uint8Array.subarray(0, uint8Array.length - 1)));

// Check a selection of various invalid UTF-8 sequences in C strings and verify
// that the `getCString` API does not cause unexpected behaviour.
for (const charBuffer of [
  Uint8Array.from([0xA0, 0xA1, 0x00]),
  Uint8Array.from([0xE2, 0x28, 0xA1, 0x00]),
  Uint8Array.from([0xE2, 0x82, 0x28, 0x00]),
  Uint8Array.from([0xF0, 0x28, 0x8C, 0xBC, 0x00]),
  Uint8Array.from([0xF0, 0x90, 0x28, 0xBC, 0x00]),
  Uint8Array.from([0xF0, 0x28, 0x8C, 0x28, 0x00]),
  Uint8Array.from([0xF8, 0xA1, 0xA1, 0xA1, 0xA1, 0x00]),
  Uint8Array.from([0xFC, 0xA1, 0xA1, 0xA1, 0xA1, 0xA1, 0x00]),
]) {
  const charBufferPointer = Deno.UnsafePointer.of(charBuffer);
  const charString = Deno.UnsafePointerView.getCString(charBufferPointer);
  const charBufferPointerArrayBuffer = new Uint8Array(Deno.UnsafePointerView.getArrayBuffer(charBufferPointer, charBuffer.length - 1));
  assertEquals(charString, new TextDecoder().decode(charBufferPointerArrayBuffer));
  assertEquals([...charBuffer.subarray(0, charBuffer.length - 1)], [...charBufferPointerArrayBuffer]);
}


const bytes = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
function hash() { return dylib.symbols.hash(bytes, bytes.byteLength); };

testOptimized(hash, () => hash());

(function cleanup() {
  dylib.close();
  throwCallback.close();
  logCallback.close();
  logManyParametersCallback.close();
  returnU8Callback.close();
  returnBufferCallback.close();
  add10Callback.close();
  nestedCallback.close();
  addToFooCallback.close();

  const resourcesPost = Deno[Deno.internal].core.resources();

  const preStr = JSON.stringify(resourcesPre, null, 2);
  const postStr = JSON.stringify(resourcesPost, null, 2);
  if (preStr !== postStr) {
    throw new Error(
      `Difference in open resources before dlopen and after closing:
Before: ${preStr}
After: ${postStr}`,
    );
  }

  console.log("Correct number of resources");
})();

function assertIsOptimized(fn) {
  const status = %GetOptimizationStatus(fn);
  assert(status & (1 << 4), `expected ${fn.name} to be optimized, but wasn't`);
}

function testOptimized(fn, callback) {
  %PrepareFunctionForOptimization(fn);
  const r1 = callback();
  if (r1 !== undefined) {
    console.log(r1);
  }
  %OptimizeFunctionOnNextCall(fn);
  const r2 = callback();
  if (r2 !== undefined) {
    console.log(r2);
  }
  assertIsOptimized(fn);
}
