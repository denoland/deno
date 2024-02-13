// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { lastIndexOfNeedle } from "./last_index_of_needle.ts";

Deno.test("[bytes] lastIndexOfNeedle1", () => {
  const i = lastIndexOfNeedle(
    new Uint8Array([0, 1, 2, 0, 1, 2, 0, 1, 3]),
    new Uint8Array([0, 1, 2]),
  );
  assertEquals(i, 3);
});

Deno.test("[bytes] lastIndexOfNeedle2", () => {
  const i = lastIndexOfNeedle(
    new Uint8Array([0, 1, 1]),
    new Uint8Array([0, 1]),
  );
  assertEquals(i, 0);
});

Deno.test("[bytes] lastIndexOfNeedle3", () => {
  const i = lastIndexOfNeedle(new Uint8Array(), new Uint8Array([0, 1]));
  assertEquals(i, -1);
});

Deno.test("[bytes] lastIndexOfNeedle with start index", () => {
  const i = lastIndexOfNeedle(
    new Uint8Array([0, 1, 2, 0, 1, 2]),
    new Uint8Array([0, 1]),
    2,
  );
  assertEquals(i, 0);
});

Deno.test("[bytes] lastIndexOfNeedle with start index 2", () => {
  const i = lastIndexOfNeedle(
    new Uint8Array([0, 1, 2, 0, 1, 2]),
    new Uint8Array([0, 1]),
    -1,
  );
  assertEquals(i, -1);
});

Deno.test("[bytes] lastIndexOfNeedle with start index greater than source index", () => {
  const i = lastIndexOfNeedle(
    new Uint8Array([0, 1, 2, 0, 1, 2]),
    new Uint8Array([0, 1]),
    7,
  );
  assertEquals(i, 3);
});

Deno.test("[bytes] lastIndexOfNeedle if needle doesn't exist within source", () => {
  const i = lastIndexOfNeedle(
    new Uint8Array([0, 1, 2, 0, 1, 2]),
    new Uint8Array([2, 3]),
  );
  assertEquals(i, -1);
});
