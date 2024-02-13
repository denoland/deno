// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBeFalsy()", () => {
  expect(false).toBeFalsy();
  expect(0).toBeFalsy();
  expect("").toBeFalsy();

  expect(true).not.toBeFalsy();
  expect(1).not.toBeFalsy();
  expect("hello").not.toBeFalsy();

  assertThrows(() => {
    expect(true).toBeFalsy();
  }, AssertionError);
  assertThrows(() => {
    expect(1).toBeFalsy();
  }, AssertionError);
  assertThrows(() => {
    expect("hello").toBeFalsy();
  }, AssertionError);

  assertThrows(() => {
    expect(false).not.toBeFalsy();
  }, AssertionError);
  assertThrows(() => {
    expect(0).not.toBeFalsy();
  }, AssertionError);
  assertThrows(() => {
    expect("").not.toBeFalsy();
  }, AssertionError);
});
