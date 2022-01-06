// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

import { assertThrows } from "../../test_util/std/testing/asserts.ts";

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
  "print_something": { parameters: [], result: "void" },
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
  "add_isize": { parameters: ["isize", "isize"], result: "isize" },
  "add_f32": { parameters: ["f32", "f32"], result: "f32" },
  "add_f64": { parameters: ["f64", "f64"], result: "f64" },
  "fill_buffer": { parameters: ["u8", "pointer", "usize"], result: "void" },
  "sleep_blocking": { parameters: ["u64"], result: "void", nonblocking: true },
  "nonblocking_buffer": {
    parameters: ["pointer", "usize"],
    result: "void",
    nonblocking: true,
  },
});

dylib.symbols.print_something();
const buffer = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
const buffer2 = new Uint8Array([9, 10]);
dylib.symbols.print_buffer(buffer, buffer.length);
dylib.symbols.print_buffer2(buffer, buffer.length, buffer2, buffer2.length);
const ptr = dylib.symbols.return_buffer();
dylib.symbols.print_buffer(ptr, 8);
const ptrView = new Deno.UnsafePointerView(ptr);
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
console.log(Boolean(dylib.symbols.is_null_ptr(ptr)));
console.log(Boolean(dylib.symbols.is_null_ptr(null)));
console.log(Boolean(dylib.symbols.is_null_ptr(Deno.UnsafePointer.of(into))));
console.log(dylib.symbols.add_u32(123, 456));
assertThrows(
  () => {
    dylib.symbols.add_u32(-1, 100);
  },
  TypeError,
  "Expected FFI argument to be an unsigned integer, but got Number(-1)",
);
assertThrows(
  () => {
    dylib.symbols.add_u32(null, 100);
  },
  TypeError,
  "Expected FFI argument to be an unsigned integer, but got Null",
);
console.log(dylib.symbols.add_i32(123, 456));
console.log(dylib.symbols.add_u64(123, 456));
console.log(dylib.symbols.add_i64(123, 456));
console.log(dylib.symbols.add_usize(123, 456));
console.log(dylib.symbols.add_isize(123, 456));
console.log(dylib.symbols.add_f32(123.123, 456.789));
console.log(dylib.symbols.add_f64(123.123, 456.789));

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

const start = performance.now();
dylib.symbols.sleep_blocking(100).then(() => {
  console.log("After");
  console.log(performance.now() - start >= 100);
  // Close after task is complete.
  cleanup();
});
console.log("Before");
console.log(performance.now() - start < 100);

function cleanup() {
  dylib.close();

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
}
