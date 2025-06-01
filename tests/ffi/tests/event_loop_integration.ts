// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const dylib = Deno.dlopen(
  libPath,
  {
    store_function: {
      parameters: ["function"],
      result: "void",
    },
    call_stored_function: {
      parameters: [],
      result: "void",
    },
    call_stored_function_thread_safe_and_log: {
      parameters: [],
      result: "void",
    },
  } as const,
);

let retry = false;
const tripleLogCallback = () => {
  console.log("Sync");
  queueMicrotask(() => {
    console.log("Async");
    callback.unref();
  });
  setTimeout(() => {
    console.log("Timeout");
    callback.unref();

    if (retry) {
      // Re-ref and retry the call to make sure re-refing works.
      console.log("RETRY THREAD SAFE");
      retry = false;
      callback.ref();
      dylib.symbols.call_stored_function_thread_safe_and_log();
    }
  }, 100);
};

const callback = Deno.UnsafeCallback.threadSafe(
  {
    parameters: [],
    result: "void",
  } as const,
  tripleLogCallback,
);

// Store function
dylib.symbols.store_function(callback.pointer);

// Synchronous callback logging
console.log("SYNCHRONOUS");
// This function only calls the callback, and does not log.
dylib.symbols.call_stored_function();
console.log("STORED_FUNCTION called");

// Wait to make sure synch logging and async logging
await new Promise((res) => setTimeout(res, 200));

// Ref once to make sure both `queueMicrotask()` and `setTimeout()`
// must resolve and unref before isolate exists.
// One ref'ing has been done by `threadSafe` constructor.
callback.ref();

console.log("THREAD SAFE");
retry = true;
// This function calls the callback and logs 'STORED_FUNCTION called'
dylib.symbols.call_stored_function_thread_safe_and_log();
