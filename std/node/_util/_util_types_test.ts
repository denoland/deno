// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import { assertStrictEquals } from "../../testing/asserts.ts";

import { isDate } from "./_util_types.ts";

const { test } = Deno;

test("New date instance with no arguments", () => {
  assertStrictEquals(isDate(new Date()), true);
});

test("New date instance with value 0", () => {
  assertStrictEquals(isDate(new Date(0)), true);
});

test("New date instance in new context", () => {
  assertStrictEquals(isDate(new (eval("Date"))()), true);
});

test("Date function is not of type Date", () => {
  assertStrictEquals(isDate(Date()), false);
});

test("Object is not of type Date", () => {
  assertStrictEquals(isDate({}), false);
});

test("Array is not of type Date", () => {
  assertStrictEquals(isDate([]), false);
});

test("Error is not of type Date", () => {
  assertStrictEquals(isDate(new Error()), false);
});

test("New object from Date prototype is not of type Date", () => {
  assertStrictEquals(isDate(Object.create(Date.prototype)), false);
});
