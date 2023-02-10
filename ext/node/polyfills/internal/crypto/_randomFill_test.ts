// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { Buffer } from "../../buffer.ts";
import randomFill, { randomFillSync } from "./_randomFill.ts";
import {
  assertEquals,
  assertNotEquals,
  assertThrows,
} from "../../../testing/asserts.ts";

const validateNonZero = (buf: Buffer) => {
  if (!buf.some((ch) => ch > 0)) throw new Error("Error");
};

const validateZero = (buf: Buffer) => {
  buf.forEach((val) => assertEquals(val, 0));
};

Deno.test("[node/crypto.randomFill]", () => {
  const buf = Buffer.alloc(10);
  const before = buf.toString("hex");

  randomFill(buf, 5, 5, (_err, bufTwo) => {
    const after = bufTwo?.toString("hex");
    assertEquals(before.slice(0, 10), after?.slice(0, 10));
  });
});

Deno.test("[node/crypto.randomFillSync]", () => {
  const buf = Buffer.alloc(10);
  const before = buf.toString("hex");

  const after = randomFillSync(buf, 5, 5);

  assertNotEquals(before, after.toString("hex"));
});

Deno.test("[node/crypto.randomFillSync] Complete fill, explicit size", () => {
  const buf = Buffer.alloc(10);
  randomFillSync(buf, undefined, 10);
  validateNonZero(buf);
});

Deno.test("[randomFillSync] Complete fill", () => {
  const buf = Buffer.alloc(10);
  randomFillSync(buf);
  validateNonZero(buf);
});

Deno.test("[node/crypto.randomFillSync] Fill beginning, explicit offset+size", () => {
  const buf = Buffer.alloc(10);
  randomFillSync(buf, 0, 5);
  validateNonZero(buf);

  const untouched = buf.slice(5);
  assertEquals(untouched.length, 5);
  validateZero(untouched);
});

Deno.test("[node/crypto.randomFillSync] Invalid offst/size", () => {
  assertThrows(() => randomFillSync(Buffer.alloc(10), 1, 10));
});
