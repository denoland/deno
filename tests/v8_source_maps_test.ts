// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

// This test demonstrates a bug:
// https://github.com/denoland/deno/issues/808
test(function evalErrorFormatted() {
  let err;
  try {
    eval("boom");
  } catch (e) {
    err = e;
  }
  assert(!!err);
  // tslint:disable-next-line:no-unused-expression
  err.stack; // This would crash if err.stack is malformed
  assertEqual(err.name, "ReferenceError");
});
