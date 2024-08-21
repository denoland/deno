// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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

Deno.test(function dependentSignalsAborted() {
  const controller = new AbortController();
  const signal1 = AbortSignal.any([controller.signal]);
  const signal2 = AbortSignal.any([signal1]);
  let eventFired = false;

  controller.signal.addEventListener("abort", () => {
    const signal3 = AbortSignal.any([signal2]);
    assert(controller.signal.aborted);
    assert(signal1.aborted);
    assert(signal2.aborted);
    assert(signal3.aborted);
    eventFired = true;
  });

  controller.abort();
  assert(eventFired, "event fired");
});
