// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { Buffer } from "node:buffer";
import { assert, libSuffix } from "./common.js";

const ops = Deno[Deno.internal].core.ops;

Deno.test("ctr initialization (napi_module_register)", {
  ignore: Deno.build.os == "windows",
}, function () {
  const path = new URL(`./module.${libSuffix}`, import.meta.url).pathname;
  const obj = ops.op_napi_open(path, {}, Buffer, reportError);
  assert(obj != null);
  assert(typeof obj === "object");
});
