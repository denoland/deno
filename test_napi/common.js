// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

export {
  assert,
  assertEquals,
  assertRejects,
} from "../test_util/std/testing/asserts.ts";
export { fromFileUrl } from "../test_util/std/path/mod.ts";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];

export function loadTestLibrary() {
  const specifier = `${targetDir}/${libPrefix}test_napi.${libSuffix}`;
  return Deno.core.ops.op_napi_open(specifier); // Internal, used in ext/node
}
