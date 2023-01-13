// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const dylib = Deno.dlopen(libPath, {
  store_function: {
    parameters: ["function"],
    result: "void",
  },
  call_stored_function: {
    parameters: [],
    result: "void",
  },
});

const callback = new Deno.UnsafeCallback(
  { parameters: [], result: "void" },
  () => {
    console.log("Callback on worker thread");
  },
);

callback.ref();

self.addEventListener("message", ({ data }) => {
  if (data === "register") {
    dylib.symbols.store_function(callback.pointer);
  } else if (data === "call") {
    dylib.symbols.call_stored_function();
  } else if (data === "unref") {
    callback.unref();
  }
  self.postMessage("done");
});
