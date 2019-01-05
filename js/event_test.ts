// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "./test_util.ts";

test(function eventInitializedWithType() {
  const type = "click";
  const event = new Event(type);

  assertEqual(event.isTrusted, false);
  assertEqual(event.target, null);
  assertEqual(event.currentTarget, null);
  assertEqual(event.type, "click");
  assertEqual(event.bubbles, false);
  assertEqual(event.cancelable, false);
});

test(function eventInitializedWithTypeAndDict() {
  const init = "submit";
  const eventInitDict = new EventInit({ bubbles: true, cancelable: true });
  const event = new Event(init, eventInitDict);

  assertEqual(event.isTrusted, false);
  assertEqual(event.target, null);
  assertEqual(event.currentTarget, null);
  assertEqual(event.type, "submit");
  assertEqual(event.bubbles, true);
  assertEqual(event.cancelable, true);
});

test(function eventComposedPathSuccess() {
  const type = "click";
  const event = new Event(type);
  const composedPath = event.composedPath();

  assertEqual(composedPath, []);
});

test(function eventStopPropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEqual(event.cancelBubble, false);
  event.stopPropagation();
  assertEqual(event.cancelBubble, true);
});

test(function eventStopImmediatePropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEqual(event.cancelBubble, false);
  assertEqual(event.cancelBubbleImmediately, false);
  event.stopImmediatePropagation();
  assertEqual(event.cancelBubble, true);
  assertEqual(event.cancelBubbleImmediately, true);
});

test(function eventPreventDefaultSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEqual(event.defaultPrevented, false);
  event.preventDefault();
  assertEqual(event.defaultPrevented, false);

  const eventInitDict = new EventInit({ bubbles: true, cancelable: true });
  const cancelableEvent = new Event(type, eventInitDict);
  assertEqual(cancelableEvent.defaultPrevented, false);
  cancelableEvent.preventDefault();
  assertEqual(cancelableEvent.defaultPrevented, true);
});
