// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEquals } from "./test_util.ts";

test(function atobSuccess(): void {
  const text = "hello world";
  const encoded = btoa(text);
  assertEquals(encoded, "aGVsbG8gd29ybGQ=");
});

test(function btoaSuccess(): void {
  const encoded = "aGVsbG8gd29ybGQ=";
  const decoded = atob(encoded);
  assertEquals(decoded, "hello world");
});

test(function btoaFailed(): void {
  const text = "你好";
  let err;
  try {
    btoa(text);
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEquals(err.name, "InvalidInput");
});

test(function textDecoder2(): void {
  // prettier-ignore
  const fixture = new Uint8Array([
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
  const decoder = new TextDecoder();
  assertEquals(decoder.decode(fixture), "𝓽𝓮𝔁𝓽");
});

test(function textDecoderASCII(): void {
  const fixture = new Uint8Array([0x89, 0x95, 0x9f, 0xbf]);
  const decoder = new TextDecoder("ascii");
  assertEquals(decoder.decode(fixture), "‰•Ÿ¿");
});

test(function textDecoderErrorEncoding(): void {
  let didThrow = false;
  try {
    new TextDecoder("foo");
  } catch (e) {
    didThrow = true;
    assertEquals(e.message, "The encoding label provided ('foo') is invalid.");
  }
  assert(didThrow);
});

test(function textEncoder2(): void {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  // prettier-ignore
  assertEquals(Array.from(encoder.encode(fixture)), [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
});

test(function textDecoderSharedUint8Array(): void {
  const ab = new SharedArrayBuffer(6);
  const dataView = new DataView(ab);
  const charCodeA = "A".charCodeAt(0);
  for (let i = 0; i < ab.byteLength; i++) {
    dataView.setUint8(i, charCodeA + i);
  }
  const ui8 = new Uint8Array(ab);
  const decoder = new TextDecoder();
  const actual = decoder.decode(ui8);
  assertEquals(actual, "ABCDEF");
});

test(function textDecoderSharedInt32Array(): void {
  const ab = new SharedArrayBuffer(8);
  const dataView = new DataView(ab);
  const charCodeA = "A".charCodeAt(0);
  for (let i = 0; i < ab.byteLength; i++) {
    dataView.setUint8(i, charCodeA + i);
  }
  const i32 = new Int32Array(ab);
  const decoder = new TextDecoder();
  const actual = decoder.decode(i32);
  assertEquals(actual, "ABCDEFGH");
});

test(function toStringShouldBeWebCompatibility(): void {
  const encoder = new TextEncoder();
  assertEquals(encoder.toString(), "[object TextEncoder]");

  const decoder = new TextDecoder();
  assertEquals(decoder.toString(), "[object TextDecoder]");
});
