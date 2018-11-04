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
  const eventInitDict = new EventInit({bubbles: true, cancelable: true});
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

  assertEqual(this._stopPropagationFlag, false);
  event.stopPropagation();
  assertEqual(this._stopPropagationFlag, true);
});

test(function eventStopImmediatePropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEqual(this._stopPropagationFlag, false);
  assertEqual(this._stopImmediatePropagationFlag, false);
  event.stopImmediatePropagation();
  assertEqual(this._stopPropagationFlag, true);
  assertEqual(this._stopImmediatePropagationFlag, true);
});

test(function eventPreventDefaultSuccess() {
  const type = "click";
  const event = new Event(type);

  assertEqual(this.defaultPrevented, false);
  event.preventDefault();
  assertEqual(this.defaultPrevented, true);
});
