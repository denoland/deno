// Copyright 2018-2025 the Deno authors. MIT license.

export { assert, assertEquals, assertRejects, assertThrows } from "@std/assert";
export { fromFileUrl } from "@std/path";
import process from "node:process";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
export const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];

export function loadTestLibrary() {
  const specifier = `${targetDir}/${libPrefix}test_napi.${libSuffix}`;

  // Internal, used in ext/node
  const module = {};
  // Pass some flag, it should be ignored, but make sure it doesn't print
  // warnings.
  process.dlopen(module, specifier, 0);
  return module.exports;
}
