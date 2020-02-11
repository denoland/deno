// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assertEquals, assert } from "./test_util.ts";

test(function eventInitializedWithType(): void {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.currentTarget, null);
  assertEquals(event.type, "click");
  assertEquals(event.bubbles, false);
  assertEquals(event.cancelable, false);
});

test(function eventInitializedWithTypeAndDict(): void {
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

test(function eventComposedPathSuccess(): void {
  const type = "click";
  const event = new Event(type);
  const composedPath = event.composedPath();

  assertEquals(composedPath, []);
});

test(function eventStopPropagationSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.cancelBubble, false);
  event.stopPropagation();
  assertEquals(event.cancelBubble, true);
});

test(function eventStopImmediatePropagationSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.cancelBubble, false);
  assertEquals(event.cancelBubbleImmediately, false);
  event.stopImmediatePropagation();
  assertEquals(event.cancelBubble, true);
  assertEquals(event.cancelBubbleImmediately, true);
});

test(function eventPreventDefaultSuccess(): void {
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

test(function eventInitializedWithNonStringType(): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
test(function eventIsTrusted(): void {
  const desc1 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc1);
  assertEquals(typeof desc1.get, "function");

  const desc2 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc2);
  assertEquals(typeof desc2!.get, "function");

  assertEquals(desc1!.get, desc2!.get);
});
