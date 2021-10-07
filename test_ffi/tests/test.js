// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const resourcesPre = Deno.resources();
const dylib = Deno.dlopen(libPath, {
  "print_something": { parameters: [], result: "void" },
  "print_buffer": { parameters: ["buffer", "usize"], result: "void" },
  "add_u32": { parameters: ["u32", "u32"], result: "u32" },
  "add_i32": { parameters: ["i32", "i32"], result: "i32" },
  "add_u64": { parameters: ["u64", "u64"], result: "u64" },
  "add_i64": { parameters: ["i64", "i64"], result: "i64" },
  "add_usize": { parameters: ["usize", "usize"], result: "usize" },
  "add_isize": { parameters: ["isize", "isize"], result: "isize" },
  "add_f32": { parameters: ["f32", "f32"], result: "f32" },
  "add_f64": { parameters: ["f64", "f64"], result: "f64" },
  "sleep_blocking": { parameters: ["u64"], result: "void", nonblocking: true },
  "nonblocking_buffer": {
    parameters: ["buffer", "usize"],
    result: "void",
    nonblocking: true,
  },
});

dylib.symbols.print_something();
const buffer = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
dylib.symbols.print_buffer(buffer, buffer.length);
console.log(dylib.symbols.add_u32(123, 456));
console.log(dylib.symbols.add_i32(123, 456));
console.log(dylib.symbols.add_u64(123, 456));
console.log(dylib.symbols.add_i64(123, 456));
console.log(dylib.symbols.add_usize(123, 456));
console.log(dylib.symbols.add_isize(123, 456));
console.log(dylib.symbols.add_f32(123.123, 456.789));
console.log(dylib.symbols.add_f64(123.123, 456.789));

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
const buffer2 = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
dylib.symbols.nonblocking_buffer(buffer2, buffer2.length).then(() => {
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
