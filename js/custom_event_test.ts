// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "./test_util.ts";

test(function customEventInitializedWithDetail() {
  const type = "touchstart";
  const detail = { message: "hello" };
  const customEventDict = new CustomEventInit({
    bubbles: true,
    cancelable: true,
    detail
  });
  const event = new CustomEvent(type, customEventDict);

  assertEqual(event.bubbles, true);
  assertEqual(event.cancelable, true);
  assertEqual(event.currentTarget, null);
  assertEqual(event.detail, detail);
  assertEqual(event.isTrusted, false);
  assertEqual(event.target, null);
  assertEqual(event.type, type);
});
