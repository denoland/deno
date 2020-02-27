// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function btoaSuccess(): void {
  const text = "hello world";
  const encoded = btoa(text);
  assert.equals(encoded, "aGVsbG8gd29ybGQ=");
});

test(function atobSuccess(): void {
  const encoded = "aGVsbG8gd29ybGQ=";
  const decoded = atob(encoded);
  assert.equals(decoded, "hello world");
});

test(function atobWithAsciiWhitespace(): void {
  const encodedList = [
    " aGVsbG8gd29ybGQ=",
    "  aGVsbG8gd29ybGQ=",
    "aGVsbG8gd29ybGQ= ",
    "aGVsbG8gd29ybGQ=\n",
    "aGVsbG\t8gd29ybGQ=",
    `aGVsbG\t8g
                d29ybGQ=`
  ];

  for (const encoded of encodedList) {
    const decoded = atob(encoded);
    assert.equals(decoded, "hello world");
  }
});

test(function atobThrows(): void {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ==");
  } catch (e) {
    threw = true;
  }
  assert(threw);
});

test(function atobThrows2(): void {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ===");
  } catch (e) {
    threw = true;
  }
  assert(threw);
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
  assert(err instanceof TypeError);
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
  assert.equals(decoder.decode(fixture), "𝓽𝓮𝔁𝓽");
});

test(function textDecoderIgnoreBOM(): void {
  // prettier-ignore
  const fixture = new Uint8Array([
    0xef, 0xbb, 0xbf,
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
  const decoder = new TextDecoder("utf-8", { ignoreBOM: true });
  assert.equals(decoder.decode(fixture), "𝓽𝓮𝔁𝓽");
});

test(function textDecoderNotBOM(): void {
  // prettier-ignore
  const fixture = new Uint8Array([
    0xef, 0xbb, 0x89,
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
  const decoder = new TextDecoder("utf-8", { ignoreBOM: true });
  assert.equals(decoder.decode(fixture), "ﻉ𝓽𝓮𝔁𝓽");
});

test(function textDecoderASCII(): void {
  const fixture = new Uint8Array([0x89, 0x95, 0x9f, 0xbf]);
  const decoder = new TextDecoder("ascii");
  assert.equals(decoder.decode(fixture), "‰•Ÿ¿");
});

test(function textDecoderErrorEncoding(): void {
  let didThrow = false;
  try {
    new TextDecoder("foo");
  } catch (e) {
    didThrow = true;
    assert.equals(e.message, "The encoding label provided ('foo') is invalid.");
  }
  assert(didThrow);
});

test(function textEncoder(): void {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  // prettier-ignore
  assert.equals(Array.from(encoder.encode(fixture)), [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
});

test(function textEncodeInto(): void {
  const fixture = "text";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(5);
  const result = encoder.encodeInto(fixture, bytes);
  assert.equals(result.read, 4);
  assert.equals(result.written, 4);
  // prettier-ignore
  assert.equals(Array.from(bytes), [
    0x74, 0x65, 0x78, 0x74, 0x00,
  ]);
});

test(function textEncodeInto2(): void {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(17);
  const result = encoder.encodeInto(fixture, bytes);
  assert.equals(result.read, 8);
  assert.equals(result.written, 16);
  // prettier-ignore
  assert.equals(Array.from(bytes), [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd, 0x00,
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
  assert.equals(actual, "ABCDEF");
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
  assert.equals(actual, "ABCDEFGH");
});

test(function toStringShouldBeWebCompatibility(): void {
  const encoder = new TextEncoder();
  assert.equals(encoder.toString(), "[object TextEncoder]");

  const decoder = new TextDecoder();
  assert.equals(decoder.toString(), "[object TextDecoder]");
});
