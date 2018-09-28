// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

// TODO(kevinkassimo): remove this once FileReader lands
// (such that we can actually access the blob contents for testing) 
// This is a copy from impl in blob.ts,
// since direct import from blob.ts won't work
function convertLineEndingsToNative(
  s: string,
  nativeLineEnding: string
): string {
  // https://w3c.github.io/FileAPI/#convert-line-endings-to-native
  let result = "";
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (c === "\n" || c === "\r") {
      result += nativeLineEnding;
    } else {
      result += c;
    }
    i += 1;
    if (c === "\r" && i < s.length && s[i] === "\n") {
      i += 1;
    }
  }
  return result;
}

test(async function blobString() {
  const b1 = new Blob(["Hello World"]);
  const str = "Test";
  const b2 = new Blob([b1, str]);
  assertEqual(b2.size, b1.size + str.length);
});

test(async function blobBuffer() {
  const buffer = new ArrayBuffer(12);
  const u8 = new Uint8Array(buffer);
  const f1 = new Float32Array(buffer);
  const b1 = new Blob([buffer, u8]);
  assertEqual(b1.size, 2 * u8.length);
  const b2 = new Blob([b1, f1]);
  assertEqual(b2.size, 3 * u8.length);
});

test(async function blobSlice() {
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

test(function blobNativeLineEnding() {
  const s1 = "\r\n,\n,\r,\r\r,\r\r\n,\n\r,\n\n,\n\r\n,\r\n\r,\r\n\n,\r\n\r\n,";
  assertEqual(
    convertLineEndingsToNative(s1, "\r\n"),
    "\r\n,\r\n,\r\n,\r\n\r\n,\r\n\r\n,\r\n\r\n,\r\n\r\n,\r\n\r\n,\r\n\r\n,\r\n\r\n,\r\n\r\n,"
  );
  assertEqual(
    convertLineEndingsToNative(s1, "\n"),
    "\n,\n,\n,\n\n,\n\n,\n\n,\n\n,\n\n,\n\n,\n\n,\n\n,"
  );
  const s2 = "a\r\n";
  assertEqual(convertLineEndingsToNative(s2, "\r\n"), "a\r\n");
  assertEqual(convertLineEndingsToNative(s2, "\n"), "a\n");
  const s3 = "a\n";
  assertEqual(convertLineEndingsToNative(s3, "\r\n"), "a\r\n");
  assertEqual(convertLineEndingsToNative(s3, "\n"), "a\n");
  const s4 = "a\r";
  assertEqual(convertLineEndingsToNative(s4, "\r\n"), "a\r\n");
  assertEqual(convertLineEndingsToNative(s4, "\n"), "a\n");
});

// TODO(qti3e) Test the stored data in a Blob after implementing FileReader API.
