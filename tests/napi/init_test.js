// Copyright 2018-2026 the Deno authors. MIT license.

import { Buffer } from "node:buffer";
import { assert, assertThrows, fromFileUrl, libSuffix } from "./common.js";
import { Worker } from "node:worker_threads";
const ops = Deno[Deno.internal].core.ops;
const noop = () => {};

// Use noops for async hooks -- this test only validates module
// initialization, not async context propagation.
const emitInit = noop;
const emitBefore = noop;
const emitAfter = noop;
const emitDestroy = noop;

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

// A legacy V8/nan addon (registered via `node_module_register` with a
// `nm_version` that is not the Node-API version) must be rejected with a clear
// error instead of crashing with a cryptic `dyld: missing symbol called` abort.
// See denoland/deno#26656.
Deno.test("legacy V8 addon is rejected with a clear error", {
  ignore: Deno.build.os == "windows",
}, function () {
  const path =
    new URL(`./module_legacy.${libSuffix}`, import.meta.url).pathname;
  assertThrows(
    () => {
      ops.op_napi_open(
        path,
        {},
        Buffer.from,
        reportError,
        emitInit,
        emitBefore,
        emitAfter,
        emitDestroy,
      );
    },
    TypeError,
    "legacy Node.js native addon API",
  );
});

// When the platform loader (dlopen / LoadLibraryExW) rejects a `.node`, the
// error must surface the underlying OS error and the resolved path, instead of
// an opaque message (e.g. a bare `LoadLibraryExW failed` on Windows that
// discards both). See denoland/deno#36622.
Deno.test("addon load failure surfaces the OS error and path", function () {
  // `common.js` exists and is readable, but is a text file rather than a shared
  // library, so the loader fails to open it.
  const path = fromFileUrl(new URL("./common.js", import.meta.url));
  const err = assertThrows(
    () => {
      ops.op_napi_open(
        path,
        {},
        Buffer.from,
        reportError,
        emitInit,
        emitBefore,
        emitAfter,
        emitDestroy,
      );
    },
    TypeError,
  );
  // The resolved path is included so that, when the cause is path length, the
  // error is self-diagnosing.
  assert(
    err.message.includes(path),
    `expected the addon path in the error, got: ${err.message}`,
  );
  assert(
    err.message.includes("path:"),
    `expected a "path:" label in the error, got: ${err.message}`,
  );
});
