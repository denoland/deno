// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toContain()", () => {
  const arr = [1, 2, 3];

  expect(arr).toContain(2);
  expect("foobarbaz").toContain("bar");

  expect(arr).not.toContain(4);
  expect("foobarbaz").not.toContain("qux");

  assertThrows(() => {
    expect(arr).toContain(4);
  }, AssertionError);
  assertThrows(() => {
    expect("foobarbaz").toContain("qux");
  }, AssertionError);

  assertThrows(() => {
    expect(arr).not.toContain(2);
  }, AssertionError);
  assertThrows(() => {
    expect("foobarbaz").not.toContain("bar");
  }, AssertionError);
});
