// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { minBy } from "./min_by.ts";

Deno.test("[collections/minBy] of array of input", () => {
  const input = [
    { name: "Kyle", age: 34 },
    { name: "John", age: 42 },
    { name: "Anna", age: 23 },
  ];

  const min = minBy(input, (i) => i.age);

  assertEquals(min, { name: "Anna", age: 23 });
});

Deno.test("[collections/minBy] of array of input with mutation", () => {
  const input = [
    { name: "Kyle", age: 34 },
    { name: "John", age: 42 },
    { name: "Anna", age: 23 },
  ];

  const min = minBy(input, (i) => i.age - 10);

  assertEquals(min, { name: "Anna", age: 23 });
});

Deno.test("[collections/minBy] of array of input with multiple min", () => {
  const input = [
    { name: "Kyle", age: 34 },
    { name: "John", age: 42 },
    { name: "Anna", age: 23 },
    { name: "Anna", age: 23 },
  ];

  const min = minBy(input, (i) => i.age);

  assertEquals(min, { name: "Anna", age: 23 });
});

Deno.test("[collections/minBy] of array of positive numbers", () => {
  const input = [2, 3, 5];

  const min = minBy(input, (i) => i);

  assertEquals(min, 2);
});

Deno.test("[collections/minBy] of array of negative numbers", () => {
  const input = [-2, -3, -5];

  const min = minBy(input, (i) => i);

  assertEquals(min, -5);
});

Deno.test("[collections/minBy] of array of strings", () => {
  const input = ["Kyle", "John", "Anna"];

  const min = minBy(input, (i: string) => i);

  assertEquals(min, "Anna");
});

Deno.test("[collections/minBy] of empty array", () => {
  const input: number[] = [];

  const min = minBy(input, (i) => i);

  assertEquals(min, undefined);
});

Deno.test("[collections/minBy] of array of numbers with multiple min", () => {
  const input = [2, 3, 5, 5];

  const min = minBy(input, (i) => i);

  assertEquals(min, 2);
});

Deno.test("[collections/minBy] of array of numbers with infinity", () => {
  const input = [2, 3, 5, -Infinity];

  const min = minBy(input, (i: number) => i);

  assertEquals(min, -Infinity);
});

Deno.test("[collections/minBy] of array of numbers with NaN", () => {
  const input = [2, 3, 5, NaN];

  const min = minBy(input, (i) => i);

  assertEquals(min, 2);
});

Deno.test("[collections/minBy] no mutation", () => {
  const input = [2, 3, 5, NaN];

  minBy(input, (i: number) => i);

  assertEquals(input, [2, 3, 5, NaN]);
});

Deno.test("[collections/minBy] empty input", () => {
  const input: Array<{ age: number }> = [];

  const min = minBy(input, (i) => i.age);

  assertEquals(min, undefined);
});

Deno.test({
  name: "[collections/sortBy] bigint",
  fn() {
    const input = [
      "9007199254740999",
      "9007199254740991",
      "9007199254740995",
    ];

    assertEquals(minBy(input, (it) => BigInt(it)), "9007199254740991");
  },
});

Deno.test({
  name: "[collections/sortBy] date",
  fn() {
    const input = [
      "February 1, 2022",
      "December 17, 1995",
      "June 12, 2012",
    ];

    assertEquals(minBy(input, (it) => new Date(it)), "December 17, 1995");
  },
});
