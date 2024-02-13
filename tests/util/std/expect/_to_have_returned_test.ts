// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { fn } from "./fn.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toHaveReturned()", () => {
  const mockFn0 = fn();
  const mockFn1 = fn(() => {
    throw new Error("foo");
  });

  mockFn0();
  try {
    mockFn1();
  } catch {
    // ignore
  }

  expect(mockFn0).toHaveReturned();

  expect(mockFn1).not.toHaveReturned();

  assertThrows(() => {
    expect(mockFn1).toHaveReturned();
  }, AssertionError);

  assertThrows(() => {
    expect(mockFn0).not.toHaveReturned();
  }, AssertionError);
});
