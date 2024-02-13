// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeGreaterThan()", () => {
  expect(11).toBeGreaterThan(10);

  expect(10).not.toBeGreaterThan(10);
  expect(9).not.toBeGreaterThan(10);

  assertThrows(() => {
    expect(10).toBeGreaterThan(10);
  }, AssertionError);
  assertThrows(() => {
    expect(9).toBeGreaterThan(10);
  });

  assertThrows(() => {
    expect(11).not.toBeGreaterThan(10);
  }, AssertionError);
});
