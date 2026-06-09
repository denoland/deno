// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals } from "./test_util.ts";

Deno.test(function basicAbortController() {
  const controller = new AbortController();
  assert(controller);
  const { signal } = controller;
  assert(signal);
  assertEquals(signal.aborted, false);
  controller.abort();
  assertEquals(signal.aborted, true);
});

Deno.test(function signalCallsOnabort() {
  const controller = new AbortController();
  const { signal } = controller;
  let called = false;
  signal.onabort = (evt) => {
    assert(evt);
    assertEquals(evt.type, "abort");
    called = true;
  };
  controller.abort();
  assert(called);
});

Deno.test(function signalEventListener() {
  const controller = new AbortController();
  const { signal } = controller;
  let called = false;
  signal.addEventListener("abort", function (ev) {
    assert(this === signal);
    assertEquals(ev.type, "abort");
    called = true;
  });
  controller.abort();
  assert(called);
});

Deno.test(function onlyAbortsOnce() {
  const controller = new AbortController();
  const { signal } = controller;
  let called = 0;
  signal.addEventListener("abort", () => called++);
  signal.onabort = () => {
    called++;
  };
  controller.abort();
  assertEquals(called, 2);
  controller.abort();
  assertEquals(called, 2);
});

Deno.test(function controllerHasProperToString() {
  const actual = Object.prototype.toString.call(new AbortController());
  assertEquals(actual, "[object AbortController]");
});

Deno.test(function abortReason() {
  const signal = AbortSignal.abort("hey!");
  assertEquals(signal.aborted, true);
  assertEquals(signal.reason, "hey!");
});

// Regression test for https://github.com/denoland/deno/issues/32858: a very
// large timeout (larger than the maximum 32-bit signed integer) must not abort
// the signal almost immediately.
Deno.test(async function timeoutLargeDelayDoesNotAbortImmediately() {
  const signal = AbortSignal.timeout(2 ** 53 - 1);
  await new Promise((resolve) => setTimeout(resolve, 100));
  assertEquals(signal.aborted, false);
});
