// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeNull()", () => {
  expect(null).toBeNull();

  expect(undefined).not.toBeNull();

  assertThrows(() => {
    expect(undefined).toBeNull();
  }, AssertionError);

  assertThrows(() => {
    expect(null).not.toBeNull();
  }, AssertionError);
});
