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

const tripleLogCallback = () => {
  console.log("Sync");
  Promise.resolve().then(() => {
    console.log("Async");
    callback.unref();
  });
  setTimeout(() => {
    console.log("Timeout");
    callback.unref();
  }, 10);
};

const callback = new Deno.UnsafeCallback(
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
dylib.symbols.call_stored_function();
console.log("STORED_FUNCTION called");

// Wait to make sure synch logging and async logging
await new Promise((res) => setTimeout(res, 100));

// Ref twice to make sure both `Promise.resolve().then()` and `setTimeout()`
// must resolve before isolate exists.
callback.ref();
callback.ref();

console.log("THREAD SAFE");
dylib.symbols.call_stored_function_thread_safe_and_log();
