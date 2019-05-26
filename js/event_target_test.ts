// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assertEquals } from "./test_util.ts";

test(function addEventListenerTest(): void {
  const document = new EventTarget();

  assertEquals(document.addEventListener("x", null, false), undefined);
  assertEquals(document.addEventListener("x", null, true), undefined);
  assertEquals(document.addEventListener("x", null), undefined);
});

test(function constructedEventTargetCanBeUsedAsExpected(): void {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e): void => {
    assertEquals(e, event);
    ++callCount;
  };

  target.addEventListener("foo", listener);

  target.dispatchEvent(event);
  assertEquals(callCount, 1);

  target.dispatchEvent(event);
  assertEquals(callCount, 2);

  target.removeEventListener("foo", listener);
  target.dispatchEvent(event);
  assertEquals(callCount, 2);
});

test(function anEventTargetCanBeSubclassed(): void {
  class NicerEventTarget extends EventTarget {
    on(type, callback?, options?): void {
      this.addEventListener(type, callback, options);
    }

    off(type, callback?, options?): void {
      this.removeEventListener(type, callback, options);
    }
  }

  const target = new NicerEventTarget();
  new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (): void => {
    ++callCount;
  };

  target.on("foo", listener);
  assertEquals(callCount, 0);

  target.off("foo", listener);
  assertEquals(callCount, 0);
});

test(function removingNullEventListenerShouldSucceed(): void {
  const document = new EventTarget();
  assertEquals(document.removeEventListener("x", null, false), undefined);
  assertEquals(document.removeEventListener("x", null, true), undefined);
  assertEquals(document.removeEventListener("x", null), undefined);
});

test(function constructedEventTargetUseObjectPrototype(): void {
  const target = new EventTarget();
  const event = new Event("toString", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e): void => {
    assertEquals(e, event);
    ++callCount;
  };

  target.addEventListener("toString", listener);

  target.dispatchEvent(event);
  assertEquals(callCount, 1);

  target.dispatchEvent(event);
  assertEquals(callCount, 2);

  target.removeEventListener("toString", listener);
  target.dispatchEvent(event);
  assertEquals(callCount, 2);
});

test(function toStringShouldBeWebCompatibility(): void {
  const target = new EventTarget();
  assertEquals(target.toString(), "[object EventTarget]");
});

test(function dispatchEventShouldNotThrowError(): void {
  let hasThrown = false;

  try {
    const target = new EventTarget();
    const event = new Event("hasOwnProperty", {
      bubbles: true,
      cancelable: false
    });
    const listener = (): void => {};
    target.addEventListener("hasOwnProperty", listener);
    target.dispatchEvent(event);
  } catch {
    hasThrown = true;
  }

  assertEquals(hasThrown, false);
});
