// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { randomFillSync, randomUUID, timingSafeEqual } from "node:crypto";
import { Buffer } from "node:buffer";
import { assert, assertEquals } from "../../unit/test_util.ts";
import { assertNotEquals } from "@std/assert";

Deno.test("[node/crypto.getRandomUUID] works the same way as Web Crypto API", () => {
  assertEquals(randomUUID().length, crypto.randomUUID().length);
  assertEquals(typeof randomUUID(), typeof crypto.randomUUID());
});

Deno.test("[node/crypto.randomFillSync] supported arguments", () => {
  const buf = new Uint8Array(10);

  assert(randomFillSync(buf));
  assert(randomFillSync(buf, 0));
  // @ts-ignore: arraybuffer arguments are valid.
  assert(randomFillSync(buf.buffer));
  assert(randomFillSync(new DataView(buf.buffer)));
});

Deno.test("[node/crypto.randomFillSync] array buffer view", () => {
  const buf = new Uint8Array(32);
  const view = new Uint8Array(buf.buffer, 8, 16);

  assert(randomFillSync(view));
  assertEquals(view.length, 16);
  assertNotEquals(view, new Uint8Array(16));
  assertEquals(buf.subarray(0, 8), new Uint8Array(8));
  assertEquals(buf.subarray(24, 32), new Uint8Array(8));
});

Deno.test("[node/crypto.timingSafeEqual] compares equal Buffer with different byteOffset", () => {
  const a = Buffer.from([212, 213]);
  const b = Buffer.from([0, 0, 0, 0, 0, 0, 0, 0, 212, 213]).subarray(8);

  assert(timingSafeEqual(a, b));
});
