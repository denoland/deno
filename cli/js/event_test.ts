// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function eventInitializedWithType(): void {
  const type = "click";
  const event = new Event(type);

  assert.equals(event.isTrusted, false);
  assert.equals(event.target, null);
  assert.equals(event.currentTarget, null);
  assert.equals(event.type, "click");
  assert.equals(event.bubbles, false);
  assert.equals(event.cancelable, false);
});

test(function eventInitializedWithTypeAndDict(): void {
  const init = "submit";
  const eventInit = { bubbles: true, cancelable: true } as EventInit;
  const event = new Event(init, eventInit);

  assert.equals(event.isTrusted, false);
  assert.equals(event.target, null);
  assert.equals(event.currentTarget, null);
  assert.equals(event.type, "submit");
  assert.equals(event.bubbles, true);
  assert.equals(event.cancelable, true);
});

test(function eventComposedPathSuccess(): void {
  const type = "click";
  const event = new Event(type);
  const composedPath = event.composedPath();

  assert.equals(composedPath, []);
});

test(function eventStopPropagationSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assert.equals(event.cancelBubble, false);
  event.stopPropagation();
  assert.equals(event.cancelBubble, true);
});

test(function eventStopImmediatePropagationSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assert.equals(event.cancelBubble, false);
  assert.equals(event.cancelBubbleImmediately, false);
  event.stopImmediatePropagation();
  assert.equals(event.cancelBubble, true);
  assert.equals(event.cancelBubbleImmediately, true);
});

test(function eventPreventDefaultSuccess(): void {
  const type = "click";
  const event = new Event(type);

  assert.equals(event.defaultPrevented, false);
  event.preventDefault();
  assert.equals(event.defaultPrevented, false);

  const eventInit = { bubbles: true, cancelable: true } as EventInit;
  const cancelableEvent = new Event(type, eventInit);
  assert.equals(cancelableEvent.defaultPrevented, false);
  cancelableEvent.preventDefault();
  assert.equals(cancelableEvent.defaultPrevented, true);
});

test(function eventInitializedWithNonStringType(): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const type: any = undefined;
  const event = new Event(type);

  assert.equals(event.isTrusted, false);
  assert.equals(event.target, null);
  assert.equals(event.currentTarget, null);
  assert.equals(event.type, "undefined");
  assert.equals(event.bubbles, false);
  assert.equals(event.cancelable, false);
});

// ref https://github.com/web-platform-tests/wpt/blob/master/dom/events/Event-isTrusted.any.js
test(function eventIsTrusted(): void {
  const desc1 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc1);
  assert.equals(typeof desc1.get, "function");

  const desc2 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc2);
  assert.equals(typeof desc2!.get, "function");

  assert.equals(desc1!.get, desc2!.get);
});
