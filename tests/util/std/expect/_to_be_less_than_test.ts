// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeLessThan()", () => {
  expect(9).toBeLessThan(10);

  expect(10).not.toBeLessThan(10);
  expect(11).not.toBeLessThan(10);

  assertThrows(() => {
    expect(10).toBeLessThan(10);
  }, AssertionError);
  assertThrows(() => {
    expect(11).toBeLessThan(10);
  }, AssertionError);

  assertThrows(() => {
    expect(9).not.toBeLessThan(10);
  }, AssertionError);
});
