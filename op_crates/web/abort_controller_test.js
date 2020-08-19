// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function assertEquals(left, right) {
  assert(left === right);
}

function basicAbortController() {
  controller = new AbortController();
  assert(controller);
  const { signal } = controller;
  assert(signal);
  assertEquals(signal.aborted, false);
  controller.abort();
  assertEquals(signal.aborted, true);
}

function signalCallsOnabort() {
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
}

function signalEventListener() {
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
}

function onlyAbortsOnce() {
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
}

function controllerHasProperToString() {
  const actual = Object.prototype.toString.call(new AbortController());
  assertEquals(actual, "[object AbortController]");
}

function main() {
  basicAbortController();
  signalCallsOnabort();
  signalEventListener();
  onlyAbortsOnce();
  controllerHasProperToString();
}

main();
