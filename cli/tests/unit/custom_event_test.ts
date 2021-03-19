// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test("customEventInitializedWithDetail", function (): void {
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

Deno.test("toStringShouldBeWebCompatibility", function (): void {
  const type = "touchstart";
  const event = new CustomEvent(type, {});
  assertEquals(event.toString(), "[object CustomEvent]");
});
