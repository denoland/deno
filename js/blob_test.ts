// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

test(function blobString() {
  const b1 = new Blob(["Hello World"]);
  const str = "Test";
  const b2 = new Blob([b1, str]);
  assertEqual(b2.size, b1.size + str.length);
});

test(function blobBuffer() {
  const buffer = new ArrayBuffer(12);
  const u8 = new Uint8Array(buffer);
  const f1 = new Float32Array(buffer);
  const b1 = new Blob([buffer, u8]);
  assertEqual(b1.size, 2 * u8.length);
  const b2 = new Blob([b1, f1]);
  assertEqual(b2.size, 3 * u8.length);
});

test(function blobSlice() {
  const blob = new Blob(["Deno", "Foo"]);
  const b1 = blob.slice(0, 3, "Text/HTML");
  assert(b1 instanceof Blob);
  assertEqual(b1.size, 3);
  assertEqual(b1.type, "text/html");
  const b2 = blob.slice(-1, 3);
  assertEqual(b2.size, 0);
  const b3 = blob.slice(100, 3);
  assertEqual(b3.size, 0);
  const b4 = blob.slice(0, 10);
  assertEqual(b4.size, blob.size);
});

// TODO(qti3e) Test the stored data in a Blob after implementing FileReader API.
