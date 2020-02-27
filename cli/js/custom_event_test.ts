// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function customEventInitializedWithDetail(): void {
  const type = "touchstart";
  const detail = { message: "hello" };
  const customEventInit = {
    bubbles: true,
    cancelable: true,
    detail
  } as CustomEventInit;
  const event = new CustomEvent(type, customEventInit);

  assert.equals(event.bubbles, true);
  assert.equals(event.cancelable, true);
  assert.equals(event.currentTarget, null);
  assert.equals(event.detail, detail);
  assert.equals(event.isTrusted, false);
  assert.equals(event.target, null);
  assert.equals(event.type, type);
});

test(function toStringShouldBeWebCompatibility(): void {
  const type = "touchstart";
  const event = new CustomEvent(type, {});
  assert.equals(event.toString(), "[object CustomEvent]");
});
