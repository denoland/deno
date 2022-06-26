// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
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
  store_function: {
    parameters: ["function"],
    result: "void",
  },
  call_stored_function: {
    parameters: [],
    result: "void",
  },
  call_stored_function_thread_safe: {
    parameters: [],
    result: "void",
  },
});

let resolveWorker;
let workerResponsePromise;

const worker = new Worker(
  new URL("./thread_safe_test_worker.js", import.meta.url).href,
  { type: "module" },
);

worker.addEventListener("message", () => {
  if (resolveWorker) {
    resolveWorker();
  }
});

const sendWorkerMessage = async (data) => {
  workerResponsePromise = new Promise((res) => {
    resolveWorker = res;
  });
  worker.postMessage(data);
  await workerResponsePromise;
};

// Test step 1: Register main thread callback, trigger on worker thread

const mainThreadCallback = new Deno.UnsafeCallback(
  { parameters: [], result: "void" },
  () => {
    console.log("Callback on main thread");
  },
);

mainThreadCallback.ref();

dylib.symbols.store_function(mainThreadCallback.pointer);

await sendWorkerMessage("call");

// Test step 2: Register on worker thread, trigger on main thread

await sendWorkerMessage("register");

dylib.symbols.call_stored_function();

// Unref both main and worker thread callbacks and terminate the wrorker: Note, the stored function pointer in lib is now dangling.

mainThreadCallback.unref();
await sendWorkerMessage("unref");
worker.terminate();

// Test step 3: Register a callback that will be the only thing left keeping the isolate from exiting.
// Rely on it to keep Deno running until the callback comes in and unrefs the callback, after which Deno should exit.

const cleanupCallback = new Deno.UnsafeCallback(
  { parameters: [], result: "void" },
  () => {
    console.log("Callback being called");
    Promise.resolve().then(() => cleanup());
  },
);

cleanupCallback.ref();

function cleanup() {
  cleanupCallback.unref();
  console.log("Isolate should now exit");
}

dylib.symbols.store_function(cleanupCallback.pointer);

console.log(
  "Calling callback, isolate should stay asleep until callback is called",
);
dylib.symbols.call_stored_function_thread_safe();
