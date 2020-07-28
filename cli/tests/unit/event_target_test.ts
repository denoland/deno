// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals } from "./test_util.ts";

unitTest(function addEventListenerTest(): void {
  const document = new EventTarget();

  assertEquals(document.addEventListener("x", null, false), undefined);
  assertEquals(document.addEventListener("x", null, true), undefined);
  assertEquals(document.addEventListener("x", null), undefined);
});

unitTest(function constructedEventTargetCanBeUsedAsExpected(): void {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e: Event): void => {
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

unitTest(function anEventTargetCanBeSubclassed(): void {
  class NicerEventTarget extends EventTarget {
    on(
      type: string,
      callback: ((e: Event) => void) | null,
      options?: AddEventListenerOptions,
    ): void {
      this.addEventListener(type, callback, options);
    }

    off(
      type: string,
      callback: ((e: Event) => void) | null,
      options?: EventListenerOptions,
    ): void {
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

unitTest(function removingNullEventListenerShouldSucceed(): void {
  const document = new EventTarget();
  assertEquals(document.removeEventListener("x", null, false), undefined);
  assertEquals(document.removeEventListener("x", null, true), undefined);
  assertEquals(document.removeEventListener("x", null), undefined);
});

unitTest(function constructedEventTargetUseObjectPrototype(): void {
  const target = new EventTarget();
  const event = new Event("toString", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e: Event): void => {
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

unitTest(function toStringShouldBeWebCompatible(): void {
  const target = new EventTarget();
  assertEquals(target.toString(), "[object EventTarget]");
});

unitTest(function dispatchEventShouldNotThrowError(): void {
  let hasThrown = false;

  try {
    const target = new EventTarget();
    const event = new Event("hasOwnProperty", {
      bubbles: true,
      cancelable: false,
    });
    const listener = (): void => {};
    target.addEventListener("hasOwnProperty", listener);
    target.dispatchEvent(event);
  } catch {
    hasThrown = true;
  }

  assertEquals(hasThrown, false);
});

unitTest(function eventTargetThisShouldDefaultToWindow(): void {
  const {
    addEventListener,
    dispatchEvent,
    removeEventListener,
  } = EventTarget.prototype;
  let n = 1;
  const event = new Event("hello");
  const listener = (): void => {
    n = 2;
  };

  addEventListener("hello", listener);
  window.dispatchEvent(event);
  assertEquals(n, 2);
  n = 1;
  removeEventListener("hello", listener);
  window.dispatchEvent(event);
  assertEquals(n, 1);

  window.addEventListener("hello", listener);
  dispatchEvent(event);
  assertEquals(n, 2);
  n = 1;
  window.removeEventListener("hello", listener);
  dispatchEvent(event);
  assertEquals(n, 1);
});

unitTest(function eventTargetShouldAcceptEventListenerObject(): void {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = {
    handleEvent(e: Event): void {
      assertEquals(e, event);
      ++callCount;
    },
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

unitTest(function eventTargetShouldAcceptAsyncFunction(): void {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e: Event): void => {
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

unitTest(
  function eventTargetShouldAcceptAsyncFunctionForEventListenerObject(): void {
    const target = new EventTarget();
    const event = new Event("foo", { bubbles: true, cancelable: false });
    let callCount = 0;

    const listener = {
      handleEvent(e: Event): void {
        assertEquals(e, event);
        ++callCount;
      },
    };

    target.addEventListener("foo", listener);

    target.dispatchEvent(event);
    assertEquals(callCount, 1);

    target.dispatchEvent(event);
    assertEquals(callCount, 2);

    target.removeEventListener("foo", listener);
    target.dispatchEvent(event);
    assertEquals(callCount, 2);
  },
);
