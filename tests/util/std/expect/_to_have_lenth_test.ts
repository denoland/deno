// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toHaveLength()", () => {
  expect([1, 2, 3]).toHaveLength(3);
  expect("abc").toHaveLength(3);

  expect([1, 2, 3]).not.toHaveLength(4);
  expect("abc").not.toHaveLength(4);

  assertThrows(() => {
    expect([1, 2, 3]).toHaveLength(4);
  }, AssertionError);
  assertThrows(() => {
    expect("abc").toHaveLength(4);
  }, AssertionError);

  assertThrows(() => {
    expect([1, 2, 3]).not.toHaveLength(3);
  }, AssertionError);
  assertThrows(() => {
    expect("abc").not.toHaveLength(3);
  }, AssertionError);
});
