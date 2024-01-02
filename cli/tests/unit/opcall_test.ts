// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../../../test_util/std/assert/mod.ts";
import { assert, assertStringIncludes, unreachable } from "./test_util.ts";

Deno.test(async function sendAsyncStackTrace() {
  try {
    await core.ops.op_error_async();
    unreachable();
  } catch (error) {
    assert(error instanceof Error);
    const s = error.stack?.toString();
    assert(s);
    assertStringIncludes(s, "opcall_test.ts");
    assertStringIncludes(s, "sendAsyncStackTrace");
    assert(
      !s.includes("ext:core"),
      "opcall stack traces should NOT include ext:core internals such as unwrapOpResult",
    );
  }
});

Deno.test(async function sendAsyncStackTraceDeferred() {
  try {
    await core.ops.op_error_async_deferred();
    unreachable();
  } catch (error) {
    assert(error instanceof Error);
    const s = error.stack?.toString();
    assert(s);
    assertStringIncludes(s, "opcall_test.ts");
    assertStringIncludes(s, "sendAsyncStackTraceDeferred");
    assert(
      !s.includes("ext:core"),
      "opcall stack traces should NOT include ext:core internals such as unwrapOpResult",
    );
  }
});

Deno.test(function syncAdd() {
  assertEquals(30, core.ops.op_add(10, 20));
});

Deno.test(async function asyncAdd() {
  assertEquals(30, await core.ops.op_add_async(10, 20));
});

// @ts-ignore This is not publicly typed namespace, but it's there for sure.
const core = Deno[Deno.internal].core;

Deno.test(async function opsAsyncBadResource() {
  try {
    const nonExistingRid = 9999;
    await core.read(
      nonExistingRid,
      new Uint8Array(0),
    );
  } catch (e) {
    if (!(e instanceof Deno.errors.BadResource)) {
      throw e;
    }
  }
});

Deno.test(function opsSyncBadResource() {
  try {
    const nonExistingRid = 9999;
    core.ops.op_read_sync(
      nonExistingRid,
      new Uint8Array(0),
    );
  } catch (e) {
    if (!(e instanceof Deno.errors.BadResource)) {
      throw e;
    }
  }
});
