// Copyright 2018-2026 the Deno authors. MIT license.

import { Buffer } from "node:buffer";
import { assert, libSuffix } from "./common.js";
import { Worker } from "node:worker_threads";
import {
  emitInit,
  emitBefore,
  emitAfter,
  emitDestroy,
} from "ext:deno_node/internal/async_hooks.ts";

const ops = Deno[Deno.internal].core.ops;
const noop = () => {};

Deno.test("ctr initialization (napi_module_register)", {
  ignore: Deno.build.os == "windows",
}, function () {
  const path = new URL(`./module.${libSuffix}`, import.meta.url).pathname;
  const obj = ops.op_napi_open(
    path,
    {},
    Buffer.from,
    reportError,
    emitInit,
    emitBefore,
    emitAfter,
    emitDestroy,
  );
  assert(obj != null);
  assert(typeof obj === "object");
});

Deno.test("ctr initialization by multiple threads (napi_module_register)", {
  ignore: Deno.build.os == "windows",
}, async function () {
  const path = new URL(`./module.${libSuffix}`, import.meta.url).pathname;
  const obj = ops.op_napi_open(
    path,
    {},
    Buffer.from,
    reportError,
    emitInit,
    emitBefore,
    emitAfter,
    emitDestroy,
  );
  assert(obj != null);
  assert(typeof obj === "object");

  const worker = new Worker(
    `
    const { Buffer } = require("node:buffer");
    const { parentPort } = require("node:worker_threads");
    const assert = require("node:assert");

    const ops = Deno[Deno.internal].core.ops;
    const noop = () => {};
    const obj = ops.op_napi_open(
      "${path}", {}, Buffer.from, reportError,
      noop, noop, noop, noop,
    );
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
