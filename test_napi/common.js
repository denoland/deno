// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

export {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "../test_util/std/testing/asserts.ts";
export { fromFileUrl } from "../test_util/std/path/mod.ts";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
export const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];

const ops = Deno[Deno.internal].core.ops;

export function loadTestLibrary() {
  const specifier = `${targetDir}/${libPrefix}test_napi.${libSuffix}`;

  // Internal, used in ext/node
  return ops.op_napi_open(specifier, {
    Buffer: {},
  });
}
