// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { delay } from "./delay.ts";
import {
  assert,
  assertInstanceOf,
  assertRejects,
  assertStrictEquals,
} from "../assert/mod.ts";

// https://dom.spec.whatwg.org/#interface-AbortSignal
function assertIsDefaultAbortReason(reason: unknown) {
  assertInstanceOf(reason, DOMException);
  assertStrictEquals(reason.name, "AbortError");
}

Deno.test("[async] delay", async function () {
  const start = new Date();
  const delayedPromise = delay(100);
  const result = await delayedPromise;
  const diff = new Date().getTime() - start.getTime();
  assert(result === undefined);
  assert(diff >= 100);
});

Deno.test("[async] delay with abort", async function () {
  const start = new Date();
  const abort = new AbortController();
  const { signal } = abort;
  const delayedPromise = delay(100, { signal });
  setTimeout(() => abort.abort(), 0);
  const cause = await assertRejects(() => delayedPromise);
  const diff = new Date().getTime() - start.getTime();
  assert(diff < 100);
  assertIsDefaultAbortReason(cause);
});

Deno.test("[async] delay with abort reason", async function (ctx) {
  async function assertRejectsReason(reason: unknown) {
    const start = new Date();
    const abort = new AbortController();
    const { signal } = abort;
    const delayedPromise = delay(100, { signal });
    setTimeout(() => abort.abort(reason), 0);
    const cause = await assertRejects(() => delayedPromise);
    const diff = new Date().getTime() - start.getTime();
    assert(diff < 100);
    assertStrictEquals(cause, reason);
  }

  await ctx.step("not-undefined values", async () => {
    await Promise.all([
      null,
      new Error("Timeout cancelled"),
      new DOMException("Timeout cancelled", "AbortError"),
      new DOMException("The signal has been aborted", "AbortError"),
    ].map(assertRejectsReason));
  });

  await ctx.step("undefined", async () => {
    const start = new Date();
    const abort = new AbortController();
    const { signal } = abort;
    const delayedPromise = delay(100, { signal });
    setTimeout(() => abort.abort(), 0);
    const cause = await assertRejects(() => delayedPromise);
    const diff = new Date().getTime() - start.getTime();
    assert(diff < 100);
    assertIsDefaultAbortReason(cause);
  });
});

Deno.test("[async] delay with non-aborted signal", async function () {
  const start = new Date();
  const abort = new AbortController();
  const { signal } = abort;
  const delayedPromise = delay(100, { signal });
  const result = await delayedPromise;
  const diff = new Date().getTime() - start.getTime();
  assert(result === undefined);
  assert(diff >= 100);
});

Deno.test("[async] delay with signal aborted after delay", async function () {
  const start = new Date();
  const abort = new AbortController();
  const { signal } = abort;
  const delayedPromise = delay(100, { signal });
  const result = await delayedPromise;
  abort.abort();
  const diff = new Date().getTime() - start.getTime();
  assert(result === undefined);
  assert(diff >= 100);
});

Deno.test("[async] delay with already aborted signal", async function () {
  const start = new Date();
  const abort = new AbortController();
  abort.abort();
  const { signal } = abort;
  const delayedPromise = delay(100, { signal });
  const cause = await assertRejects(() => delayedPromise);
  const diff = new Date().getTime() - start.getTime();
  assert(diff < 100);
  assertIsDefaultAbortReason(cause);
});
