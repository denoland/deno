// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toHaveProperty()", () => {
  expect({ a: 1 }).toHaveProperty("a");
  expect({ a: 1 }).toHaveProperty("a", 1);
  expect({ a: { b: 1 } }).toHaveProperty("a.b", 1);
  expect({ a: { b: 1 } }).toHaveProperty(["a", "b"], 1);
  expect({ a: { b: { c: { d: 5 } } } }).toHaveProperty("a.b.c", { d: 5 });
  expect({ a: { b: { c: { d: 5 } } } }).toHaveProperty("a.b.c.d", 5);

  expect({ a: { b: { c: { d: 5 } } } }).not.toHaveProperty("a.b.c", { d: 6 });

  assertThrows(() => {
    expect({ a: { b: { c: { d: 5 } } } }).toHaveProperty("a.b.c", { d: 6 });
  }, AssertionError);

  assertThrows(() => {
    expect({ a: { b: { c: { d: 5 } } } }).not.toHaveProperty("a.b.c", { d: 5 });
  }, AssertionError);
});
