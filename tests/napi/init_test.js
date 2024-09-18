// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { Buffer } from "node:buffer";
import { assert, libSuffix } from "./common.js";
import { Worker } from "node:worker_threads";

const ops = Deno[Deno.internal].core.ops;

Deno.test("ctr initialization (napi_module_register)", {
  ignore: Deno.build.os == "windows",
}, function () {
  const path = new URL(`./module.${libSuffix}`, import.meta.url).pathname;
  const obj = ops.op_napi_open(path, {}, Buffer, reportError);
  assert(obj != null);
  assert(typeof obj === "object");
});

Deno.test("ctr initialization by multiple threads (napi_module_register)", {
  ignore: Deno.build.os == "windows",
}, async function () {
  const path = new URL(`./module.${libSuffix}`, import.meta.url).pathname;
  const obj = ops.op_napi_open(path, {}, Buffer, reportError);
  const common = import.meta.resolve("./common.js");
  assert(obj != null);
  assert(typeof obj === "object");

  const worker = new Worker(
    `
    import { Buffer } from "node:buffer";
    import { parentPort } from "node:worker_threads";
    import { assert } from "${common}";
    
    const ops = Deno[Deno.internal].core.ops;
    const obj = ops.op_napi_open("${path}", {}, Buffer, reportError);
    assert(obj != null);
    assert(typeof obj === "object");
    parentPort.postMessage("ok");
    `,
    {
      eval: true,
    },
  );

  const p = Promise.withResolvers();
  worker.on("message", (_m) => {
    p.resolve();
  });

  await p.promise;
});
