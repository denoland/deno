// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

test(function atobSuccess() {
  const text = "hello world";
  const encoded = btoa(text);
  assertEqual(encoded, "aGVsbG8gd29ybGQ=");
});

test(function btoaSuccess() {
  const encoded = "aGVsbG8gd29ybGQ=";
  const decoded = atob(encoded);
  assertEqual(decoded, "hello world");
});

test(function btoaFailed() {
  const text = "你好";
  let err;
  try {
    btoa(text);
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEqual(err.name, "InvalidInput");
});
