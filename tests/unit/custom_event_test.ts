// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(function customEventInitializedWithDetail() {
  const type = "touchstart";
  const detail = { message: "hello" };
  const customEventInit = {
    bubbles: true,
    cancelable: true,
    detail,
  } as CustomEventInit;
  const event = new CustomEvent(type, customEventInit);

  assertEquals(event.bubbles, true);
  assertEquals(event.cancelable, true);
  assertEquals(event.currentTarget, null);
  assertEquals(event.detail, detail);
  assertEquals(event.isTrusted, false);
  assertEquals(event.target, null);
  assertEquals(event.type, type);
});

Deno.test(function toStringShouldBeWebCompatibility() {
  const type = "touchstart";
  const event = new CustomEvent(type, {});
  assertEquals(event.toString(), "[object CustomEvent]");
});
