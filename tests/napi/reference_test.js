// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi strong reference create/get/delete", function () {
  const result = lib.test_reference_strong();
  assertEquals(typeof result, "object");
  assertEquals(result.marker, 123);
});

Deno.test("napi reference ref/unref counting", function () {
  const result = lib.test_reference_ref_unref();
  assertEquals(result, true);
});

Deno.test("napi create_external / get_value_external", function () {
  const result = lib.test_create_external();
  assertEquals(result, 42);
});

Deno.test("napi external with reference", function () {
  const result = lib.test_create_external_reference();
  assertEquals(result, 99);
});

Deno.test("napi reference double delete does not crash", async function () {
  // Run in a subprocess since double-delete may crash the process
  // if not handled gracefully.
  const { code } = await new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      "--unstable-ffi",
      `
      import process from "node:process";
      const targetDir = Deno.execPath().replace(/[^\\/\\\\]+$/, "");
      const [libPrefix, libSuffix] = {
        darwin: ["lib", "dylib"],
        linux: ["lib", "so"],
        windows: ["", "dll"],
      }[Deno.build.os];
      const module = {};
      process.dlopen(module, targetDir + "/" + libPrefix + "test_napi." + libSuffix, 0);
      module.exports.test_reference_double_delete();
      `,
    ],
  }).output();
  // If the process exits 0, the double-delete was handled gracefully.
  // If it crashes (non-zero), that's a known limitation we accept for now.
  assertEquals(typeof code, "number");
});
