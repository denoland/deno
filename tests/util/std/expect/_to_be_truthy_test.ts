// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeTruthy()", () => {
  expect(1).toBeTruthy();
  expect("hello").toBeTruthy();
  expect({}).toBeTruthy();

  expect(0).not.toBeTruthy();
  expect("").not.toBeTruthy();
  expect(null).not.toBeTruthy();
  expect(undefined).not.toBeTruthy();

  assertThrows(() => {
    expect(0).toBeTruthy();
  }, AssertionError);
  assertThrows(() => {
    expect("").toBeTruthy();
  }, AssertionError);
  assertThrows(() => {
    expect(null).toBeTruthy();
  }, AssertionError);
  assertThrows(() => {
    expect(undefined).toBeTruthy();
  }, AssertionError);

  assertThrows(() => {
    expect(1).not.toBeTruthy();
  }, AssertionError);
  assertThrows(() => {
    expect("hello").not.toBeTruthy();
  }, AssertionError);
  assertThrows(() => {
    expect({}).not.toBeTruthy();
  }, AssertionError);
});
