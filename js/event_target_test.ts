// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util";

if (!Event) {
  let Event = function() {};
}

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

    dispatch(type, detail) {
      this.dispatchEvent(new CustomEvent(type, { detail }));
    }
  }

  const target = new NicerEventTarget();
  const event = new Event("foo", { bubbles: true, cancelable: false });
  const detail = "some data";
  let callCount = 0;

  function listener(e) {
    assertEqual(e.detail, detail);
    ++callCount;
  }

  target.on("foo", listener);

  target.dispatch("foo", detail);
  assertEqual(callCount, 1);

  target.dispatch("foo", detail);
  assertEqual(callCount, 2);

  target.off("foo", listener);
  target.dispatch("foo", detail);
  assertEqual(callCount, 2);
});

test(function dispatchEventReturnValueAffectedByPreventDefault() {
  const eventType = "foo";
  const target = new EventTarget();
  const parent = new EventTarget();
  let defaultPrevented;
  let returnValue;
  parent.addEventListener(eventType, e => {}, true);
  target.addEventListener(
    eventType,
    e => {
      evt.preventDefault();
      defaultPrevented = evt.defaultPrevented;
      returnValue = evt.returnValue;
    },
    true
  );
  target.addEventListener(eventType, e => {}, true);
  const evt = new Event("Event");
  evt.initEvent(eventType, true, true);
  assert(parent.dispatchEvent(evt));
  assert(!target.dispatchEvent(evt));
  assert(defaultPrevented);
  assert(!returnValue);
});

test(function dispatchEventReturnValueAffectedByReturnValueProperty() {
  const eventType = "foo";
  const target = new EventTarget();
  const parent = new EventTarget();
  let defaultPrevented;
  let returnValue;
  parent.addEventListener(eventType, e => {}, true);
  target.addEventListener(
    eventType,
    e => {
      evt.returnValue = false;
      defaultPrevented = evt.defaultPrevented;
      returnValue = evt.returnValue;
    },
    true
  );
  target.addEventListener(eventType, e => {}, true);
  const evt = new Event("Event");
  evt.initEvent(eventType, true, true);
  assert(parent.dispatchEvent(evt));
  assert(!target.dispatchEvent(evt));
  assert(defaultPrevented);
  assert(!returnValue);
});

test(function removingNullEventListenerShouldSucceed() {
  const document = new EventTarget();
  assertEqual(document.removeEventListener("x", null, false), undefined);
  assertEqual(document.removeEventListener("x", null, true), undefined);
  assertEqual(document.removeEventListener("x", null), undefined);
});
