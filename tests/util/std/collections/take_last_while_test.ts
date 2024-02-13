// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { takeLastWhile } from "./take_last_while.ts";

Deno.test("[collections/takeLastWhile] Num array", () => {
  const arr = [1, 2, 3, 4, 5, 6];
  const actual = takeLastWhile(arr, (i) => i !== 4);

  assertEquals(actual, [5, 6]);
});

Deno.test("[collections/takeLastWhile] Add two to each num in predicate", () => {
  const arr = [1, 2, 3, 4, 5, 6];
  const actual = takeLastWhile(arr, (i) => i + 2 !== 6);

  assertEquals(actual, [5, 6]);
});

Deno.test("[collections/takeLastWhile] Negatives", () => {
  const arr = [-1, -2, -3, -4, -5, -6];

  const actual = takeLastWhile(arr, (i) => i < -4);
  assertEquals(actual, [-5, -6]);
});

Deno.test("[collections/takeLastWhile] No mutation", () => {
  const arr = [1, 2, 3, 4, 5, 6];

  const actual = takeLastWhile(arr, (i) => i !== 4);
  assertEquals(actual, [5, 6]);
  assertEquals(arr, [1, 2, 3, 4, 5, 6]);
});

Deno.test("[collections/takeLastWhile] Empty input array returns empty array", () => {
  const arr: number[] = [];

  const actual = takeLastWhile(arr, (i) => i > 4);

  assertEquals(actual, []);
});

Deno.test("[collections/takeLastWhile] Returns empty array when the last element doesn't match the predicate", () => {
  const arr = [1, 2, 3, 4];

  const actual = takeLastWhile(arr, (i) => i !== 4);

  assertEquals(actual, []);
});

Deno.test("[collections/takeLastWhile] Returns the same array when all elements match the predicate", () => {
  const arr = [1, 2, 3, 4];

  const actual = takeLastWhile(arr, (i) => i !== 400);

  assertEquals(actual, [1, 2, 3, 4]);
});
