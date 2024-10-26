// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "./test_util.ts";

Deno.test(function addEventListenerTest() {
  const document = new EventTarget();

  assertEquals(document.addEventListener("x", null, false), undefined);
  assertEquals(document.addEventListener("x", null, true), undefined);
  assertEquals(document.addEventListener("x", null), undefined);
});

Deno.test(function constructedEventTargetCanBeUsedAsExpected() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e: Event) => {
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

Deno.test(function anEventTargetCanBeSubclassed() {
  class NicerEventTarget extends EventTarget {
    on(
      type: string,
      callback: ((e: Event) => void) | null,
      options?: AddEventListenerOptions,
    ) {
      this.addEventListener(type, callback, options);
    }

    off(
      type: string,
      callback: ((e: Event) => void) | null,
      options?: EventListenerOptions,
    ) {
      this.removeEventListener(type, callback, options);
    }
  }

  const target = new NicerEventTarget();
  new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = () => {
    ++callCount;
  };

  target.on("foo", listener);
  assertEquals(callCount, 0);

  target.off("foo", listener);
  assertEquals(callCount, 0);
});

Deno.test(function removeEventListenerTest() {
  const target = new EventTarget();
  let callCount = 0;
  const listener = () => {
    ++callCount;
  };

  target.addEventListener("incr", listener, true);

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 1);

  // Should not remove the listener because useCapture does not match
  target.removeEventListener("incr", listener, false);

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 2);

  // Should remove the listener because useCapture matches
  target.removeEventListener("incr", listener, true);

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 2);

  // Only the capture setting matters to removeEventListener
  target.addEventListener("incr", listener, { passive: true });

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 3);

  // Should not remove the listener because useCapture does not match
  target.removeEventListener("incr", listener, { capture: true });
  target.removeEventListener("incr", listener, true);

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 4);

  // Should remove the listener because useCapture matches
  target.removeEventListener("incr", listener);

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 4);

  // Again, should remove the listener because useCapture matches
  target.addEventListener("incr", listener, { passive: true });
  target.removeEventListener("incr", listener, false);

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 4);

  // Again, should remove the listener because useCapture matches
  target.addEventListener("incr", listener, { passive: true });
  target.removeEventListener("incr", listener, { capture: false });

  target.dispatchEvent(new Event("incr"));
  assertEquals(callCount, 4);
});

Deno.test(function removingNullEventListenerShouldSucceed() {
  const document = new EventTarget();
  assertEquals(document.removeEventListener("x", null, false), undefined);
  assertEquals(document.removeEventListener("x", null, true), undefined);
  assertEquals(document.removeEventListener("x", null), undefined);
});

Deno.test(function constructedEventTargetUseObjectPrototype() {
  const target = new EventTarget();
  const event = new Event("toString", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e: Event) => {
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

Deno.test(function toStringShouldBeWebCompatible() {
  const target = new EventTarget();
  assertEquals(target.toString(), "[object EventTarget]");
});

Deno.test(function dispatchEventShouldNotThrowError() {
  let hasThrown = false;

  try {
    const target = new EventTarget();
    const event = new Event("hasOwnProperty", {
      bubbles: true,
      cancelable: false,
    });
    const listener = () => {};
    target.addEventListener("hasOwnProperty", listener);
    target.dispatchEvent(event);
  } catch {
    hasThrown = true;
  }

  assertEquals(hasThrown, false);
});

Deno.test(function eventTargetThisShouldDefaultToWindow() {
  const {
    addEventListener,
    dispatchEvent,
    removeEventListener,
  } = EventTarget.prototype;
  let n = 1;
  const event = new Event("hello");
  const listener = () => {
    n = 2;
  };

  addEventListener("hello", listener);
  globalThis.dispatchEvent(event);
  assertEquals(n, 2);
  n = 1;
  removeEventListener("hello", listener);
  globalThis.dispatchEvent(event);
  assertEquals(n, 1);

  globalThis.addEventListener("hello", listener);
  dispatchEvent(event);
  assertEquals(n, 2);
  n = 1;
  globalThis.removeEventListener("hello", listener);
  dispatchEvent(event);
  assertEquals(n, 1);
});

Deno.test(function eventTargetShouldAcceptEventListenerObject() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = {
    handleEvent(e: Event) {
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

Deno.test(function eventTargetShouldAcceptAsyncFunction() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e: Event) => {
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

Deno.test(
  function eventTargetShouldAcceptAsyncFunctionForEventListenerObject() {
    const target = new EventTarget();
    const event = new Event("foo", { bubbles: true, cancelable: false });
    let callCount = 0;

    const listener = {
      handleEvent(e: Event) {
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
Deno.test(function eventTargetDispatchShouldSetTargetNoListener() {
  const target = new EventTarget();
  const event = new Event("foo");
  assertEquals(event.target, null);
  target.dispatchEvent(event);
  assertEquals(event.target, target);
});

Deno.test(function eventTargetDispatchShouldSetTargetInListener() {
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

Deno.test(function eventTargetDispatchShouldFireCurrentListenersOnly() {
  const target = new EventTarget();
  const event = new Event("foo");
  let callCount = 0;
  target.addEventListener("foo", () => {
    ++callCount;
    target.addEventListener("foo", () => {
      ++callCount;
    });
  });
  target.dispatchEvent(event);
  assertEquals(callCount, 1);
});

Deno.test(function eventTargetAddEventListenerGlobalAbort() {
  return new Promise((resolve) => {
    const c = new AbortController();

    c.signal.addEventListener("abort", () => resolve());
    addEventListener("test", () => {}, { signal: c.signal });
    c.abort();
  });
});

Deno.test(function eventTargetBrandChecking() {
  const self = {};

  assertThrows(
    () => {
      EventTarget.prototype.addEventListener.call(self, "test", null);
    },
    TypeError,
  );

  assertThrows(
    () => {
      EventTarget.prototype.removeEventListener.call(self, "test", null);
    },
    TypeError,
  );

  assertThrows(
    () => {
      EventTarget.prototype.dispatchEvent.call(self, new Event("test"));
    },
    TypeError,
  );
});
