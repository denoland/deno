// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeInstanceOf", () => {
  expect(new Error()).toBeInstanceOf(Error);
  expect(new Error()).toBeInstanceOf(Object);

  expect(new Error()).not.toBeInstanceOf(String);

  assertThrows(() => {
    expect(new Error()).toBeInstanceOf(String);
  }, AssertionError);

  assertThrows(() => {
    expect(new Error()).not.toBeInstanceOf(Error);
  }, AssertionError);
  assertThrows(() => {
    expect(new Error()).not.toBeInstanceOf(Object);
  }, AssertionError);
});
