// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function assertEquals(left, right) {
  assert(left === right);
}

function assertThrows(fn) {
  let error = null;
  try {
    fn();
  } catch (error_) {
    error = error_;
  }
  if (error == null) {
    throw new Error("Didn't throw.");
  }
  return error;
}

function basicAbortController() {
  const controller = new AbortController();
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

function abortSignalIllegalConstructor() {
  const error = assertThrows(() => new AbortSignal());
  assert(error instanceof TypeError);
  assertEquals(error.message, "Illegal constructor.");
}

function abortSignalEventOrder() {
  const arr = [];
  const controller = new AbortController();
  const { signal } = controller;
  signal.addEventListener("abort", () => arr.push(1));
  signal.onabort = () => arr.push(2);
  signal.addEventListener("abort", () => arr.push(3));
  controller.abort();
  assertEquals(arr[0], 1);
  assertEquals(arr[1], 2);
  assertEquals(arr[2], 3);
}

function abortSignalEventOrderComplex() {
  const arr = [];
  const controller = new AbortController();
  const { signal } = controller;
  signal.addEventListener("abort", () => arr.push(1));
  signal.onabort = () => {
    throw new Error();
  };
  signal.addEventListener("abort", () => arr.push(3));
  signal.onabort = () => arr.push(2);
  controller.abort();
  assertEquals(arr[0], 1);
  assertEquals(arr[1], 2);
  assertEquals(arr[2], 3);
}

function abortSignalHandlerLocation() {
  const controller = new AbortController();
  const { signal } = controller;
  const abortHandler = Object.getOwnPropertyDescriptor(signal, "onabort");
  assertEquals(abortHandler, undefined);
}
function abortSignalLength() {
  const controller = new AbortController();
  const { signal } = controller;
  assertEquals(signal.constructor.length, 0);
}
function main() {
  basicAbortController();
  signalCallsOnabort();
  signalEventListener();
  onlyAbortsOnce();
  controllerHasProperToString();
  abortSignalIllegalConstructor();
  abortSignalEventOrder();
  abortSignalEventOrderComplex();
  abortSignalHandlerLocation();
  abortSignalLength();
}

main();
