// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

// Run using cargo test or `--v8-options=--allow-natives-syntax`

import { assertThrows } from "../../test_util/std/testing/asserts.ts";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

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
  call_stored_function_thread_safe: {
    parameters: [],
    result: "void",
    callback: true,
  },
  store_function: {
    parameters: ["function"],
    result: "void",
  },
  call_stored_function: {
    parameters: [],
    result: "void",
    callback: true,
  },
});
const { symbols } = dylib;

const callback = new Deno.UnsafeCallback(
  {
    parameters: [],
    result: "void",
  },
  () => {
    console.log("Calling");
    callback.unref();
  },
);

callback.ref();

symbols.store_function(callback.pointer);
symbols.call_stored_function_thread_safe();
