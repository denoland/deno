// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function eventInitializedWithType() {
  const type = "click";
  const event = new Event(type);

  assert(event.isTrusted === false);
  assert(event.target === null);
  assert(event.currentTarget === null);
  assert(event.type === "click");
  assert(event.bubbles === false);
  assert(event.cancelable === false);
}

function eventInitializedWithTypeAndDict() {
  const init = "submit";
  const eventInit = { bubbles: true, cancelable: true };
  const event = new Event(init, eventInit);

  assert(event.isTrusted === false);
  assert(event.target === null);
  assert(event.currentTarget === null);
  assert(event.type === "submit");
  assert(event.bubbles === true);
  assert(event.cancelable === true);
}

function eventComposedPathSuccess() {
  const type = "click";
  const event = new Event(type);
  const composedPath = event.composedPath();

  assert(composedPath.length === 0);
}

function eventStopPropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assert(event.cancelBubble === false);
  event.stopPropagation();
  assert(event.cancelBubble === true);
}

function eventStopImmediatePropagationSuccess() {
  const type = "click";
  const event = new Event(type);

  assert(event.cancelBubble === false);
  event.stopImmediatePropagation();
  assert(event.cancelBubble === true);
}

function eventPreventDefaultSuccess() {
  const type = "click";
  const event = new Event(type);

  assert(event.defaultPrevented === false);
  event.preventDefault();
  assert(event.defaultPrevented === false);

  const eventInit = { bubbles: true, cancelable: true };
  const cancelableEvent = new Event(type, eventInit);
  assert(cancelableEvent.defaultPrevented === false);
  cancelableEvent.preventDefault();
  assert(cancelableEvent.defaultPrevented === true);
}

function eventInitializedWithNonStringType() {
  const type = undefined;
  const event = new Event(type);

  assert(event.isTrusted === false);
  assert(event.target === null);
  assert(event.currentTarget === null);
  assert(event.type === "undefined");
  assert(event.bubbles === false);
  assert(event.cancelable === false);
}

// ref https://github.com/web-platform-tests/wpt/blob/master/dom/events/Event-isTrusted.any.js
function eventIsTrusted() {
  const desc1 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc1);
  assert(typeof desc1.get === "function");

  const desc2 = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(desc2);
  assert(typeof desc2.get === "function");

  assert(desc1.get === desc2.get);
}

function eventIsTrustedGetterName() {
  const { get } = Object.getOwnPropertyDescriptor(new Event("x"), "isTrusted");
  assert(get.name === "get isTrusted");
  try {
    Reflect.construct(get);
    throw new Error("Should not have reached here");
  } catch (e) {
    assert(e.message.includes("not a constructor"));
  }
}
function eventAbortSignal() {
  let count = 0;
  function handler() {
    count++;
  }
  const et = new EventTarget();
  const controller = new AbortController();
  et.addEventListener("test", handler, { signal: controller.signal });
  et.dispatchEvent(new Event("test"));
  assert(count === 1);
  et.dispatchEvent(new Event("test"));
  assert(count === 2);
  controller.abort();
  et.dispatchEvent(new Event("test"));
  assert(count === 2);
  et.addEventListener("test", handler, { signal: controller.signal });
  et.dispatchEvent(new Event("test"));
  assert(count === 2);
}
function main() {
  eventInitializedWithType();
  eventInitializedWithTypeAndDict();
  eventComposedPathSuccess();
  eventStopPropagationSuccess();
  eventStopImmediatePropagationSuccess();
  eventPreventDefaultSuccess();
  eventInitializedWithNonStringType();
  eventIsTrusted();
  eventIsTrustedGetterName();
  eventAbortSignal();
}

main();
