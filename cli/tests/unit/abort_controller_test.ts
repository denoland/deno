import { assert, assertEquals } from "./test_util.ts";

Deno.test("basicAbortController", function () {
  const controller = new AbortController();
  assert(controller);
  const { signal } = controller;
  assert(signal);
  assertEquals(signal.aborted, false);
  controller.abort();
  assertEquals(signal.aborted, true);
});

Deno.test("signalCallsOnabort", function () {
  const controller = new AbortController();
  const { signal } = controller;
  let called = false;
  signal.onabort = (evt): void => {
    assert(evt);
    assertEquals(evt.type, "abort");
    called = true;
  };
  controller.abort();
  assert(called);
});

Deno.test("signalEventListener", function () {
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

Deno.test("onlyAbortsOnce", function () {
  const controller = new AbortController();
  const { signal } = controller;
  let called = 0;
  signal.addEventListener("abort", () => called++);
  signal.onabort = (): void => {
    called++;
  };
  controller.abort();
  assertEquals(called, 2);
  controller.abort();
  assertEquals(called, 2);
});

Deno.test("controllerHasProperToString", function () {
  const actual = Object.prototype.toString.call(new AbortController());
  assertEquals(actual, "[object AbortController]");
});
