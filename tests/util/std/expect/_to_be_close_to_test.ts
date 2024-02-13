// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeCloseTo()", () => {
  expect(0.2 + 0.1).toBeCloseTo(0.3);
  expect(0.2 + 0.1).toBeCloseTo(0.3, 5);
  expect(0.2 + 0.1).toBeCloseTo(0.3, 15);

  expect(0.2 + 0.11).not.toBeCloseTo(0.3);
  expect(0.2 + 0.1).not.toBeCloseTo(0.3, 16);

  assertThrows(() => {
    expect(0.2 + 0.11).toBeCloseTo(0.3);
  }, AssertionError);

  assertThrows(() => {
    expect(0.2 + 0.1).not.toBeCloseTo(0.3);
  });
});
