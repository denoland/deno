// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";

unitTest(function eventInitializedWithType(): void {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.currentTarget, null);
  assertEquals(event.type, "click");
  assertEquals(event.bubbles, false);
  assertEquals(event.cancelable, false);
});

unitTest(function eventInitializedWithTypeAndDict(): void {
  const init = "submit";
  const eventInit = { bubbles: true, cancelable: true } as EventInit;
  const event = new Event(init, eventInit);

  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.currentTarget, null);
  assertEquals(event.type, "submit");
  assertEquals(event.bubbles, true);
  assertEquals(event.cancelable, true);
});

unitTest(function eventComposedPathSuccess(): void {
  const type = "click";
  const event = new Event(type);
  const composedPath = event.composedPath();

  assertEquals(composedPath, []);
});

unitTest(function eventStopPropagationSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.cancelBubble, false);
  event.stopPropagation();
  assertEquals(event.cancelBubble, true);
});

unitTest(function eventStopImmediatePropagationSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.cancelBubble, false);
  event.stopImmediatePropagation();
  assertEquals(event.cancelBubble, true);
});

unitTest(function eventPreventDefaultSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.defaultPrevented, false);
  event.preventDefault();
  assertEquals(event.defaultPrevented, false);

  const eventInit = { bubbles: true, cancelable: true } as EventInit;
  const cancelableEvent = new Event(type, eventInit);
  assertEquals(cancelableEvent.defaultPrevented, false);
  cancelableEvent.preventDefault();
  assertEquals(cancelableEvent.defaultPrevented, true);
});

unitTest(function eventInitializedWithNonStringType(): void {
  // deno-lint-ignore no-explicit-any
  const type: any = undefined;
  const event = new Event(type);

  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.currentTarget, null);
  assertEquals(event.type, "undefined");
  assertEquals(event.bubbles, false);
  assertEquals(event.cancelable, false);
});

// ref https://github.com/web-platform-tests/wpt/blob/master/dom/events/Event-isTrusted.any.js
unitTest(function eventIsTrusted(): void {
  const desc1 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc1);
  assertEquals(typeof desc1.get, "function");

  const desc2 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc2);
  assertEquals(typeof desc2!.get, "function");

  assertEquals(desc1!.get, desc2!.get);
});

unitTest(function eventInspectOutput(): void {
  // deno-lint-ignore no-explicit-any
  const cases: Array<[any, (event: any) => string]> = [
    [
      new Event("test"),
      (event: Event) =>
        `Event {\n  bubbles: false,\n  cancelable: false,\n  composed: false,\n  currentTarget: null,\n  defaultPrevented: false,\n  eventPhase: 0,\n  target: null,\n  timeStamp: ${event.timeStamp},\n  type: "test"\n}`,
    ],
    [
      new ErrorEvent("error"),
      (event: Event) =>
        `ErrorEvent {\n  bubbles: false,\n  cancelable: false,\n  composed: false,\n  currentTarget: null,\n  defaultPrevented: false,\n  eventPhase: 0,\n  target: null,\n  timeStamp: ${event.timeStamp},\n  type: "error",\n  message: "",\n  filename: "",\n  lineno: 0,\n  colno: 0,\n  error: null\n}`,
    ],
    [
      new CloseEvent("close"),
      (event: Event) =>
        `CloseEvent {\n  bubbles: false,\n  cancelable: false,\n  composed: false,\n  currentTarget: null,\n  defaultPrevented: false,\n  eventPhase: 0,\n  target: null,\n  timeStamp: ${event.timeStamp},\n  type: "close",\n  wasClean: false,\n  code: 0,\n  reason: ""\n}`,
    ],
    [
      new CustomEvent("custom"),
      (event: Event) =>
        `CustomEvent {\n  bubbles: false,\n  cancelable: false,\n  composed: false,\n  currentTarget: null,\n  defaultPrevented: false,\n  eventPhase: 0,\n  target: null,\n  timeStamp: ${event.timeStamp},\n  type: "custom",\n  detail: undefined\n}`,
    ],
    [
      new ProgressEvent("progress"),
      (event: Event) =>
        `ProgressEvent {\n  bubbles: false,\n  cancelable: false,\n  composed: false,\n  currentTarget: null,\n  defaultPrevented: false,\n  eventPhase: 0,\n  target: null,\n  timeStamp: ${event.timeStamp},\n  type: "progress",\n  lengthComputable: false,\n  loaded: 0,\n  total: 0\n}`,
    ],
  ];

  for (const [event, outputProvider] of cases) {
    assertEquals(Deno.inspect(event), outputProvider(event));
  }
});
