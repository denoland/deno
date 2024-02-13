// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { dropWhile } from "./drop_while.ts";

Deno.test("[collections/dropWhile] Array", () => {
  const arr = [1, 2, 3, 4, 5, 6];
  const actual = dropWhile(arr, (i) => i !== 2);

  assertEquals(actual, [2, 3, 4, 5, 6]);
});

Deno.test("[collections/dropWhile] Add two to each num in predicate", () => {
  const arr = [1, 2, 3, 4, 5, 6];
  const actual = dropWhile(arr, (i) => i + 2 !== 6);

  assertEquals(actual, [4, 5, 6]);
});

Deno.test("[collections/dropWhile] Negatives", () => {
  const arr = [-5, -6];

  const actual = dropWhile(arr, (i) => i < -4);
  assertEquals(actual, []);
});

Deno.test("[collections/dropWhile] No mutation", () => {
  const arr = [1, 2, 3, 4, 5, 6];

  const actual = dropWhile(arr, (i) => i !== 4);
  assertEquals(actual, [4, 5, 6]);
  assertEquals(arr, [1, 2, 3, 4, 5, 6]);
});

Deno.test("[collections/dropWhile] Empty input array returns empty array", () => {
  const arr: number[] = [];

  const actual = dropWhile(arr, (i) => i > 4);

  assertEquals(actual, []);
});

Deno.test("[collections/dropWhile] Returns empty array when the last element doesn't match the predicate", () => {
  const arr = [1, 2, 3, 4];

  const actual = dropWhile(arr, (i) => i !== 4);

  assertEquals(actual, [4]);
});

Deno.test("[collections/dropWhile] Returns the same array when all elements match the predicate", () => {
  const arr = [1, 2, 3, 4];

  const actual = dropWhile(arr, (i) => i !== 400);

  assertEquals(actual, []);
});
