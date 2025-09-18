// Copyright 2018-2025 the Deno authors. MIT license.
import * as crypto from "node:crypto";
import { assertEquals, assertThrows } from "@std/assert";
import { Buffer } from "node:buffer";

Deno.test("timingSafeEqual ArrayBuffer and TypedArray", async () => {
  const data = new TextEncoder().encode("foo"); // Uint8Array

  const hash = await crypto.subtle.digest("SHA-256", data); // ArrayBuffer
  const ui8a = new Uint8Array(hash); // Uint8Array

  // @ts-ignore crypto.timingSafeEqual accepts ArrayBuffer
  const eq = crypto.timingSafeEqual(hash, ui8a);
  assertEquals(eq, true);
});

Deno.test("timingSafeEqual rejects non ArrayBuffer/TypedArray", () => {
  const str = "foo";
  const buf = Buffer.from(str);
  const i = 123;

  assertThrows(
    () => {
      // @ts-expect-error testing invalid input
      crypto.timingSafeEqual(str, buf);
    },
    'TypeError: The "buf1" argument must be an instance of Buffer, ArrayBuffer, TypedArray, or DataView.' +
      "Received type string ('foo')",
  );

  assertThrows(
    () => {
      // @ts-expect-error testing invalid input
      crypto.timingSafeEqual(buf, i);
    },
    'TypeError: The "buf2" argument must be an instance of Buffer, ArrayBuffer, TypedArray, or DataView.' +
      "Received type number (123)",
  );
});
