// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "./test_util.ts";

test(function addEventListenerTest() {
  const document = new EventTarget();

  assertEqual(document.addEventListener("x", null, false), undefined);
  assertEqual(document.addEventListener("x", null, true), undefined);
  assertEqual(document.addEventListener("x", null), undefined);
});

test(function constructedEventTargetCanBeUsedAsExpected() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  function listener(e) {
    assertEqual(e, event);
    ++callCount;
  }

  target.addEventListener("foo", listener);

  target.dispatchEvent(event);
  assertEqual(callCount, 1);

  target.dispatchEvent(event);
  assertEqual(callCount, 2);

  target.removeEventListener("foo", listener);
  target.dispatchEvent(event);
  assertEqual(callCount, 2);
});

test(function anEventTargetCanBeSubclassed() {
  class NicerEventTarget extends EventTarget {
    on(type, listener?, options?) {
      this.addEventListener(type, listener, options);
    }

    off(type, callback?, options?) {
      this.removeEventListener(type, callback, options);
    }
  }

  const target = new NicerEventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  function listener() {
    ++callCount;
  }

  target.on("foo", listener);
  assertEqual(callCount, 0);

  target.off("foo", listener);
  assertEqual(callCount, 0);
});

test(function removingNullEventListenerShouldSucceed() {
  const document = new EventTarget();
  assertEqual(document.removeEventListener("x", null, false), undefined);
  assertEqual(document.removeEventListener("x", null, true), undefined);
  assertEqual(document.removeEventListener("x", null), undefined);
});
