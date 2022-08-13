// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

// Run using cargo test or `--v8-options=--allow-natives-syntax`

import { assertEquals } from "https://deno.land/std@0.149.0/testing/asserts.ts";
import {
  assertThrows,
} from "../../test_util/std/testing/asserts.ts";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const resourcesPre = Deno.resources();

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

const dylib = Deno.dlopen(libPath, {
  "printSomething": {
    name: "print_something",
    parameters: [],
    result: "void",
  },
  "print_buffer": { parameters: ["pointer", "usize"], result: "void" },
  "print_buffer2": {
    parameters: ["pointer", "usize", "pointer", "usize"],
    result: "void",
  },
  "return_buffer": { parameters: [], result: "pointer" },
  "is_null_ptr": { parameters: ["pointer"], result: "u8" },
  "add_u32": { parameters: ["u32", "u32"], result: "u32" },
  "add_i32": { parameters: ["i32", "i32"], result: "i32" },
  "add_u64": { parameters: ["u64", "u64"], result: "u64" },
  "add_i64": { parameters: ["i64", "i64"], result: "i64" },
  "add_usize": { parameters: ["usize", "usize"], result: "usize" },
  "add_usize_fast": { parameters: ["usize", "usize"], result: "u32" },
  "add_isize": { parameters: ["isize", "isize"], result: "isize" },
  "add_f32": { parameters: ["f32", "f32"], result: "f32" },
  "add_f64": { parameters: ["f64", "f64"], result: "f64" },
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
  "fill_buffer": { parameters: ["u8", "pointer", "usize"], result: "void" },
  "sleep_nonblocking": {
    name: "sleep_blocking",
    parameters: ["u64"],
    result: "void",
    nonblocking: true,
  },
  "sleep_blocking": { parameters: ["u64"], result: "void" },
  "nonblocking_buffer": {
    parameters: ["pointer", "usize"],
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
    callback: true,
  },
  call_stored_function_2: {
    parameters: ["u8"],
    result: "void",
    callback: true,
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

const status = %GetOptimizationStatus(returnBuffer);
if (!(status & (1 << 4))) {
  throw new Error("returnBuffer is not optimized");
}

dylib.symbols.print_buffer(ptr0, 8);
const ptrView = new Deno.UnsafePointerView(ptr0);
const into = new Uint8Array(6);
const into2 = new Uint8Array(3);
const into2ptr = Deno.UnsafePointer.of(into2);
const into2ptrView = new Deno.UnsafePointerView(into2ptr);
const into3 = new Uint8Array(3);
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
console.log(Boolean(dylib.symbols.is_null_ptr(ptr0)));
console.log(Boolean(dylib.symbols.is_null_ptr(null)));
console.log(Boolean(dylib.symbols.is_null_ptr(Deno.UnsafePointer.of(into))));
const emptyBuffer = new BigUint64Array(0);
console.log(Boolean(dylib.symbols.is_null_ptr(emptyBuffer)));
const emptySlice = into.subarray(6);
console.log(Boolean(dylib.symbols.is_null_ptr(emptySlice)));

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

%PrepareFunctionForOptimization(addU32Fast);
console.log(addU32Fast(123, 456));
%OptimizeFunctionOnNextCall(addU32Fast);
console.log(addU32Fast(123, 456));

function addU64Fast(a, b) { return add_usize_fast(a, b); };
%PrepareFunctionForOptimization(addU64Fast);
console.log(addU64Fast(2, 3));
%OptimizeFunctionOnNextCall(addU64Fast);
console.log(addU64Fast(2, 3));

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

// Test non blocking calls

function deferred() {
  let methods;
  const promise = new Promise((resolve, reject) => {
    methods = {
      async resolve(value) {
        await value;
        resolve(value);
      },
      reject(reason) {
        reject(reason);
      },
    };
  });
  return Object.assign(promise, methods);
}

const promise = deferred();
const buffer3 = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
dylib.symbols.nonblocking_buffer(buffer3, buffer3.length).then(() => {
  promise.resolve();
});
await promise;

let start = performance.now();
dylib.symbols.sleep_blocking(100);
console.log("After sleep_blocking");
console.log(performance.now() - start >= 100);

start = performance.now();
const promise_2 = dylib.symbols.sleep_nonblocking(100).then(() => {
  console.log("After");
  console.log(performance.now() - start >= 100);
});
console.log("Before");
console.log(performance.now() - start < 100);

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
  result: "pointer",
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
console.log("Thread safe call counter:", counter);
addToFooCallback.ref();
await dylib.symbols.call_fn_ptr_thread_safe(addToFooCallback.pointer);
addToFooCallback.unref();
logCallback.ref();
await dylib.symbols.call_fn_ptr_thread_safe(logCallback.pointer);
logCallback.unref();
console.log("Thread safe call counter:", counter);
returnU8Callback.ref();
await dylib.symbols.call_fn_ptr_return_u8_thread_safe(returnU8Callback.pointer);

// Test statics
console.log("Static u32:", dylib.symbols.static_u32);
console.log("Static i64:", dylib.symbols.static_i64);
console.log(
  "Static ptr:",
  typeof dylib.symbols.static_ptr === "number",
);
const view = new Deno.UnsafePointerView(dylib.symbols.static_ptr);
console.log("Static ptr value:", view.getUint32());

const arrayBuffer = view.getArrayBuffer(4);
const uint32Array = new Uint32Array(arrayBuffer);
console.log("arrayBuffer.byteLength:", arrayBuffer.byteLength);
console.log("uint32Array.length:", uint32Array.length);
console.log("uint32Array[0]:", uint32Array[0]);
uint32Array[0] = 55; // MUTATES!
console.log("uint32Array[0] after mutation:", uint32Array[0]);
console.log("Static ptr value after mutation:", view.getUint32());

// Test non-UTF-8 characters

const charView = new Deno.UnsafePointerView(dylib.symbols.static_char);

const charArrayBuffer = charView.getArrayBuffer(14);
const uint8Array = new Uint8Array(charArrayBuffer);
assertEquals([...uint8Array], [
  0xC0, 0xC1, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
  0x00
]);

try {
  assertThrows(() => charView.getCString(), TypeError, "Invalid CString pointer, not valid UTF-8");
} catch (_err) {
  console.log("Invalid UTF-8 characters to `v8::String`:", charView.getCString());
}

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

  const resourcesPost = Deno.resources();

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