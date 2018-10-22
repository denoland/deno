// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util";

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
  const eventInitDict = new EventInit(true, true);
  const event = new Event(init, eventInitDict);

  assertEqual(event.isTrusted, false);
  assertEqual(event.target, null);
  assertEqual(event.currentTarget, null);
  assertEqual(event.type, "click");
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

  assertEqual(this.cancelBubble, false);
  event.stopPropagation();
  assertEqual(this.cancelBubble, true);
});

test(function eventStopImmediatePropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEqual(this.cancelBubble, false);
  event.stopImmediatePropagation();
  assertEqual(this.cancelBubble, true);
});

test(function eventPreventDefaultSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEqual(this.returnValue, false);
  assertEqual(this.defaultPrevented, false);
  event.preventDefault();
  assertEqual(this.returnValue, true);
  assertEqual(this.defaultPrevented, true);
});

test(function eventInitEventDispatchedSuccess() {
  const type = "click";
  const event = new Event(type);
  event.dispatch = true;

  // assert nothing happens?
  assertEqual(event.initEvent("submit"), undefined);
});

test(function eventInitEventSuccess() {
  const type = "submit";
  const event = new Event(type);

  // assert nothing happens?
  assertEqual(event.initEvent("submit", true, true), undefined);
});
