// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test("addEventListenerTest", function (): void {
  const document = new EventTarget();

  assertEquals(document.addEventListener("x", null, false), undefined);
  assertEquals(document.addEventListener("x", null, true), undefined);
  assertEquals(document.addEventListener("x", null), undefined);
});

Deno.test("constructedEventTargetCanBeUsedAsExpected", function (): void {
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

Deno.test("anEventTargetCanBeSubclassed", function (): void {
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

Deno.test("removingNullEventListenerShouldSucceed", function (): void {
  const document = new EventTarget();
  assertEquals(document.removeEventListener("x", null, false), undefined);
  assertEquals(document.removeEventListener("x", null, true), undefined);
  assertEquals(document.removeEventListener("x", null), undefined);
});

Deno.test("constructedEventTargetUseObjectPrototype", function (): void {
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

Deno.test("toStringShouldBeWebCompatible", function (): void {
  const target = new EventTarget();
  assertEquals(target.toString(), "[object EventTarget]");
});

Deno.test("dispatchEventShouldNotThrowError", function (): void {
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

Deno.test("eventTargetThisShouldDefaultToWindow", function (): void {
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

Deno.test("eventTargetShouldAcceptEventListenerObject", function (): void {
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

Deno.test("eventTargetShouldAcceptAsyncFunction", function (): void {
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

Deno.test("eventTargetShouldAcceptAsyncFunctionForEventListenerObject", function (): void {
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
Deno.test("eventTargetDispatchShouldSetTargetNoListener", function (): void {
  const target = new EventTarget();
  const event = new Event("foo");
  assertEquals(event.target, null);
  target.dispatchEvent(event);
  assertEquals(event.target, target);
});

Deno.test("eventTargetDispatchShouldSetTargetInListener", function (): void {
  const target = new EventTarget();
  const event = new Event("foo");
  assertEquals(event.target, null);
  let called = false;
  target.addEventListener("foo", (e) => {
    assertEquals(e.target, target);
    called = true;
  });
  target.dispatchEvent(event);
  assertEquals(called, true);
});
