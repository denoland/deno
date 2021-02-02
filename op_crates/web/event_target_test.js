"use strict";
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function addEventListenerTest() {
  const document = new EventTarget();

  assert(document.addEventListener("x", null, false) === undefined);
  assert(document.addEventListener("x", null, true) === undefined);
  assert(document.addEventListener("x", null) === undefined);
}

function constructedEventTargetCanBeUsedAsExpected() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e) => {
    assert(e === event);
    ++callCount;
  };

  target.addEventListener("foo", listener);

  target.dispatchEvent(event);
  assert(callCount === 1);

  target.dispatchEvent(event);
  assert(callCount === 2);

  target.removeEventListener("foo", listener);
  target.dispatchEvent(event);
  assert(callCount === 2);
}

function anEventTargetCanBeSubclassed() {
  class NicerEventTarget extends EventTarget {
    on(
      type,
      callback,
      options,
    ) {
      this.addEventListener(type, callback, options);
    }

    off(
      type,
      callback,
      options,
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
  assert(callCount === 0);

  target.off("foo", listener);
  assert(callCount === 0);
}

function removingNullEventListenerShouldSucceed() {
  const document = new EventTarget();
  assert(document.removeEventListener("x", null, false) === undefined);
  assert(document.removeEventListener("x", null, true) === undefined);
  assert(document.removeEventListener("x", null) === undefined);
}

function constructedEventTargetUseObjectPrototype() {
  const target = new EventTarget();
  const event = new Event("toString", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e) => {
    assert(e === event);
    ++callCount;
  };

  target.addEventListener("toString", listener);

  target.dispatchEvent(event);
  assert(callCount === 1);

  target.dispatchEvent(event);
  assert(callCount === 2);

  target.removeEventListener("toString", listener);
  target.dispatchEvent(event);
  assert(callCount === 2);
}

function toStringShouldBeWebCompatible() {
  const target = new EventTarget();
  assert(target.toString() === "[object EventTarget]");
}

function dispatchEventShouldNotThrowError() {
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

  assert(hasThrown === false);
}

function eventTargetThisShouldDefaultToWindow() {
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
  assert(n === 2);
  n = 1;
  removeEventListener("hello", listener);
  globalThis.dispatchEvent(event);
  assert(n === 1);

  globalThis.addEventListener("hello", listener);
  dispatchEvent(event);
  assert(n === 2);
  n = 1;
  globalThis.removeEventListener("hello", listener);
  dispatchEvent(event);
  assert(n === 1);
}

function eventTargetShouldAcceptEventListenerObject() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = {
    handleEvent(e) {
      assert(e === event);
      ++callCount;
    },
  };

  target.addEventListener("foo", listener);

  target.dispatchEvent(event);
  assert(callCount === 1);

  target.dispatchEvent(event);
  assert(callCount === 2);

  target.removeEventListener("foo", listener);
  target.dispatchEvent(event);
  assert(callCount === 2);
}

function eventTargetShouldAcceptAsyncFunction() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = (e) => {
    assert(e === event);
    ++callCount;
  };

  target.addEventListener("foo", listener);

  target.dispatchEvent(event);
  assert(callCount === 1);

  target.dispatchEvent(event);
  assert(callCount === 2);

  target.removeEventListener("foo", listener);
  target.dispatchEvent(event);
  assert(callCount === 2);
}

function eventTargetShouldAcceptAsyncFunctionForEventListenerObject() {
  const target = new EventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  let callCount = 0;

  const listener = {
    handleEvent(e) {
      assert(e === event);
      ++callCount;
    },
  };

  target.addEventListener("foo", listener);

  target.dispatchEvent(event);
  assert(callCount === 1);

  target.dispatchEvent(event);
  assert(callCount === 2);

  target.removeEventListener("foo", listener);
  target.dispatchEvent(event);
  assert(callCount === 2);
}

function main() {
  globalThis.__bootstrap.eventTarget.setEventTargetData(globalThis);
  addEventListenerTest();
  constructedEventTargetCanBeUsedAsExpected();
  anEventTargetCanBeSubclassed();
  removingNullEventListenerShouldSucceed();
  constructedEventTargetUseObjectPrototype();
  toStringShouldBeWebCompatible();
  dispatchEventShouldNotThrowError();
  eventTargetThisShouldDefaultToWindow();
  eventTargetShouldAcceptEventListenerObject();
  eventTargetShouldAcceptAsyncFunction();
  eventTargetShouldAcceptAsyncFunctionForEventListenerObject();
}

main();
