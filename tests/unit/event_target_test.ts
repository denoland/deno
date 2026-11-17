// Copyright 2018-2026 the Deno authors. MIT license.

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

Deno.test(function removeEventListenerWithNullOptions() {
  const target = new EventTarget();
  let callCount = 0;
  const listener = () => {
    ++callCount;
  };

  // null options should be treated as capture: false
  target.addEventListener("test", listener);
  // @ts-expect-error: testing runtime behavior with null (not in TS types but valid per DOM spec)
  target.removeEventListener("test", listener, null);
  target.dispatchEvent(new Event("test"));
  assertEquals(callCount, 0);

  // null options should not remove a capture listener
  target.addEventListener("test", listener, true);
  // @ts-expect-error: testing runtime behavior with null (not in TS types but valid per DOM spec)
  target.removeEventListener("test", listener, null);
  target.dispatchEvent(new Event("test"));
  assertEquals(callCount, 1);

  // cleanup
  target.removeEventListener("test", listener, true);
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
  return new Promise<void>((resolve) => {
    const c = new AbortController();

    c.signal.addEventListener("abort", () => resolve());
    addEventListener("test", () => {}, { signal: c.signal });
    c.abort();
  });
});

Deno.test(function eventTargetManualRemoveCleansAbortSignalListener() {
  const target = new EventTarget();
  const controller = new AbortController();
  const listener = () => {};

  target.addEventListener("test", listener, { signal: controller.signal });
  target.removeEventListener("test", listener);

  let removeCount = 0;
  const originalRemoveEventListener = target.removeEventListener;
  target.removeEventListener = function (...args) {
    removeCount++;
    return originalRemoveEventListener.apply(this, args);
  };
  controller.abort();

  assertEquals(removeCount, 0);
});

Deno.test(function eventTargetOnceListenerCleansAbortSignalListener() {
  const target = new EventTarget();
  const controller = new AbortController();
  const listener = () => {};

  target.addEventListener("test", listener, {
    once: true,
    signal: controller.signal,
  });
  target.dispatchEvent(new Event("test"));

  let removeCount = 0;
  const originalRemoveEventListener = target.removeEventListener;
  target.removeEventListener = function (...args) {
    removeCount++;
    return originalRemoveEventListener.apply(this, args);
  };
  controller.abort();

  assertEquals(removeCount, 0);
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

Deno.test(function targetCaptureListenersRunBeforeNonCaptureListeners() {
  for (const bubbles of [false, true]) {
    const target = new EventTarget();
    const calls: string[] = [];
    target.addEventListener("test", () => calls.push("bubble1"));
    target.addEventListener("test", () => calls.push("capture1"), true);
    target.addEventListener("test", () => calls.push("bubble2"));
    target.addEventListener("test", () => calls.push("capture2"), {
      capture: true,
    });
    target.dispatchEvent(new Event("test", { bubbles }));
    assertEquals(calls, ["capture1", "capture2", "bubble1", "bubble2"]);
  }
});

Deno.test(function targetCaptureAndNonCaptureUseAtTargetPhase() {
  const target = new EventTarget();
  const event = new Event("test");
  const calls: number[] = [];
  const listener = (e: Event) => {
    assertEquals(e.currentTarget, target);
    assertEquals(e.target, target);
    calls.push(e.eventPhase);
  };
  target.addEventListener("test", listener);
  target.addEventListener("test", listener, true);
  target.dispatchEvent(event);
  assertEquals(calls, [Event.AT_TARGET, Event.AT_TARGET]);
  assertEquals(event.eventPhase, Event.NONE);
  assertEquals(event.currentTarget, null);
});

Deno.test(function targetCaptureOnceDoesNotRemoveNonCaptureRegistration() {
  const target = new EventTarget();
  let calls = 0;
  const listener = () => calls++;
  target.addEventListener("test", listener);
  target.addEventListener("test", listener, { capture: true, once: true });
  target.dispatchEvent(new Event("test"));
  target.dispatchEvent(new Event("test"));
  assertEquals(calls, 3);
});

Deno.test(function targetCaptureCanRemoveNonCaptureListenerBeforeInvocation() {
  const target = new EventTarget();
  const calls: string[] = [];
  const bubble = () => calls.push("bubble");
  target.addEventListener("test", bubble);
  target.addEventListener("test", () => {
    calls.push("capture");
    target.removeEventListener("test", bubble);
  }, true);
  target.dispatchEvent(new Event("test"));
  assertEquals(calls, ["capture"]);
});

Deno.test(function targetCaptureAddsListenersForTheAppropriatePhase() {
  const target = new EventTarget();
  const calls: string[] = [];
  const addedCapture = () => calls.push("added capture");
  const addedBubble = () => calls.push("added bubble");
  target.addEventListener("test", () => {
    calls.push("capture");
    target.addEventListener("test", addedCapture, true);
    target.addEventListener("test", addedBubble);
  }, true);
  target.dispatchEvent(new Event("test"));
  assertEquals(calls, ["capture", "added bubble"]);
  calls.length = 0;
  target.dispatchEvent(new Event("test"));
  assertEquals(calls, ["capture", "added capture", "added bubble"]);
});

Deno.test(function targetCaptureStopImmediatePropagationStopsBothGroups() {
  const target = new EventTarget();
  const calls: string[] = [];
  target.addEventListener("test", () => calls.push("bubble"));
  target.addEventListener("test", (event) => {
    calls.push("capture");
    event.stopImmediatePropagation();
  }, { capture: true, once: true });
  target.addEventListener("test", () => calls.push("second capture"), true);
  const event = new Event("test");
  target.dispatchEvent(event);
  assertEquals(calls, ["capture"]);
  calls.length = 0;
  target.dispatchEvent(event);
  assertEquals(calls, ["second capture", "bubble"]);
});

Deno.test(function targetCaptureStopPropagationPreservesItsListenerGroup() {
  const target = new EventTarget();
  const calls: string[] = [];
  target.addEventListener("test", () => calls.push("bubble"));
  target.addEventListener("test", (event) => {
    calls.push("capture");
    event.stopPropagation();
  }, true);
  target.addEventListener("test", () => calls.push("second capture"), true);
  target.dispatchEvent(new Event("test"));
  assertEquals(calls, ["capture", "second capture"]);
});
