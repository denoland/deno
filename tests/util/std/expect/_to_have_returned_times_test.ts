// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { fn } from "./fn.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toHaveReturnedTimes()", () => {
  const mockFn = fn();

  mockFn();
  mockFn();

  expect(mockFn).toHaveReturnedTimes(2);

  expect(mockFn).not.toHaveReturnedTimes(1);

  assertThrows(() => {
    expect(mockFn).toHaveReturnedTimes(1);
  }, AssertionError);

  assertThrows(() => {
    expect(mockFn).not.toHaveReturnedTimes(2);
  }, AssertionError);
});
