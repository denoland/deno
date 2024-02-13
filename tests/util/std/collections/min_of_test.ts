// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { minOf } from "./min_of.ts";
import { assertEquals } from "../assert/mod.ts";

Deno.test("[collections/minOf] Regular min", () => {
  const array = [5, 18, 35, 120];

  const actual = minOf(array, (i) => i);
  assertEquals(actual, 5);
});

Deno.test("[collections/minOf] Mixed negatives and positives numbers", () => {
  const array = [-32, -18, 140, 36];

  const actual = minOf(array, (i) => i);
  assertEquals(actual, -32);
});

Deno.test("[collections/minOf] Negatives numbers", () => {
  const array = [-32, -18, -140, -36];

  const actual = minOf(array, (i) => i);
  assertEquals(actual, -140);
});

Deno.test("[collections/minOf] BigInt regular min", () => {
  const array = [BigInt(5), BigInt(18), BigInt(35), BigInt(120)];

  const actual = minOf(array, (i) => i);
  assertEquals(actual, BigInt(5));
});

Deno.test("[collections/minOf] BigInt negatives numbers", () => {
  const array = [BigInt(-32), BigInt(-18), BigInt(-140), BigInt(-36)];

  const actual = minOf(array, (i) => i);
  assertEquals(actual, BigInt(-140));
});

Deno.test("[collection/minOf] On object properties", () => {
  const object = [
    { name: "mustard", count: 2 },
    { name: "soy", count: 4 },
    { name: "tomato", count: 32 },
  ];

  const actual = minOf(object, (i) => i.count);
  assertEquals(actual, 2);
});

Deno.test("[collection/minOf] On mixed object properties", () => {
  const object = [
    { name: "mustard", count: -2 },
    { name: "soy", count: 4 },
    { name: "tomato", count: -32 },
  ];

  const actual = minOf(object, (i) => i.count);
  assertEquals(actual, -32);
});

Deno.test("[collections/minOf] No mutation", () => {
  const array = [1, 2, 3, 4];

  minOf(array, (i) => i + 2);

  assertEquals(array, [1, 2, 3, 4]);
});

Deno.test("[collections/minOf] Empty array results in undefined", () => {
  const array: number[] = [];

  const actual = minOf(array, (i) => i);
  assertEquals(actual, undefined);
});

Deno.test("[collections/minOf] NaN and Infinity", () => {
  const array = [
    1,
    2,
    Number.POSITIVE_INFINITY,
    3,
    4,
    Number.NEGATIVE_INFINITY,
    5,
    6,
    Number.NaN,
    7,
    8,
  ];

  const actual = minOf(array, (i) => i);
  assertEquals(actual, NaN);
});

Deno.test("[collections/minOf] Minus infinity", () => {
  const array = [1, 2, -Infinity, 3, 4, 5, 6, 7, 8];

  const actual = minOf(array, (i) => i);

  assertEquals(actual, -Infinity);
});
