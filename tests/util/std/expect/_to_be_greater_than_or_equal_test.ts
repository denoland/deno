// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeGreaterThanOrEqual()", () => {
  expect(10).toBeGreaterThanOrEqual(10);
  expect(11).toBeGreaterThanOrEqual(10);

  expect(9).not.toBeGreaterThanOrEqual(10);

  assertThrows(() => {
    expect(9).toBeGreaterThanOrEqual(10);
  }, AssertionError);

  assertThrows(() => {
    expect(11).not.toBeGreaterThanOrEqual(10);
  }, AssertionError);
});
