// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assertEquals } from "./test_util.ts";

test(function eventInitializedWithType() {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.currentTarget, null);
  assertEquals(event.type, "click");
  assertEquals(event.bubbles, false);
  assertEquals(event.cancelable, false);
});

test(function eventInitializedWithTypeAndDict() {
  const init = "submit";
  const eventInitDict = new EventInit({ bubbles: true, cancelable: true });
  const event = new Event(init, eventInitDict);

  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.currentTarget, null);
  assertEquals(event.type, "submit");
  assertEquals(event.bubbles, true);
  assertEquals(event.cancelable, true);
});

test(function eventComposedPathSuccess() {
  const type = "click";
  const event = new Event(type);
  const composedPath = event.composedPath();

  assertEquals(composedPath, []);
});

test(function eventStopPropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.cancelBubble, false);
  event.stopPropagation();
  assertEquals(event.cancelBubble, true);
});

test(function eventStopImmediatePropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.cancelBubble, false);
  assertEquals(event.cancelBubbleImmediately, false);
  event.stopImmediatePropagation();
  assertEquals(event.cancelBubble, true);
  assertEquals(event.cancelBubbleImmediately, true);
});

test(function eventPreventDefaultSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEquals(event.defaultPrevented, false);
  event.preventDefault();
  assertEquals(event.defaultPrevented, false);

  const eventInitDict = new EventInit({ bubbles: true, cancelable: true });
  const cancelableEvent = new Event(type, eventInitDict);
  assertEquals(cancelableEvent.defaultPrevented, false);
  cancelableEvent.preventDefault();
  assertEquals(cancelableEvent.defaultPrevented, true);
});

test(function eventInitializedWithNonStringType() {
  const type = undefined;
  const event = new Event(type);

  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.currentTarget, null);
  assertEquals(event.type, "undefined");
  assertEquals(event.bubbles, false);
  assertEquals(event.cancelable, false);
});
