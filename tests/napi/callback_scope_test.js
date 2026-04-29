// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";
import { AsyncLocalStorage } from "node:async_hooks";
import { createRequire } from "node:module";

const lib = loadTestLibrary();

Deno.test("napi callback scope open and close", function () {
  assertEquals(lib.test_callback_scope(), true);
});

Deno.test("napi make_callback with async context", function () {
  const result = lib.test_make_callback_with_async_context(() => 42);
  assertEquals(result, 42);
});

Deno.test("napi async context lifecycle", function () {
  // Tests napi_async_init with and without resource, and napi_async_destroy
  assertEquals(lib.test_async_context_lifecycle(), true);
});

Deno.test("napi make_callback with real async context", function () {
  const result = lib.test_make_callback_with_real_context(() => 99);
  assertEquals(result, 99);
});

// Verify async context propagation: AsyncLocalStorage should be
// visible inside napi_make_callback.
Deno.test("napi make_callback propagates async context", function () {
  const als = new AsyncLocalStorage();
  const result = als.run("test-value", () => {
    return lib.test_make_callback_with_real_context(() => {
      return als.getStore();
    });
  });
  assertEquals(result, "test-value");
});

// Load the NAPI module through require() to exercise the .node loader path
// in 01_require.js (which wires up the real async hooks functions).
Deno.test({
  name: "napi module loaded via require()",
  ignore: Deno.build.os === "windows",
  fn() {
    const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
    const { libPrefix, libSuffix } = {
      darwin: { libPrefix: "lib", libSuffix: "dylib" },
      linux: { libPrefix: "lib", libSuffix: "so" },
      windows: { libPrefix: "", libSuffix: "dll" },
    }[Deno.build.os];
    const nativeLib = `${targetDir}/${libPrefix}test_napi.${libSuffix}`;
    // Copy the native lib to a .node file so require() uses
    // Module._extensions[".node"] -> op_napi_open with real async hooks.
    const nodePath = `${targetDir}/test_napi.node`;
    try {
      Deno.removeSync(nodePath);
    } catch { /* ignore */ }
    Deno.copyFileSync(nativeLib, nodePath);

    try {
      const require = createRequire(import.meta.url);
      const mod = require(nodePath);
      assertEquals(mod.test_callback_scope(), true);

      // Verify async context works through the require()-loaded module
      const als = new AsyncLocalStorage();
      const val = als.run(123, () => {
        return mod.test_make_callback_with_real_context(() => als.getStore());
      });
      assertEquals(val, 123);
    } finally {
      try {
        Deno.removeSync(nodePath);
      } catch { /* ignore */ }
    }
  },
});
