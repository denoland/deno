// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function assertArrayEquals(a1, a2) {
  if (a1.length !== a2.length) throw Error("assert");

  for (const index in a1) {
    if (a1[index] !== a2[index]) {
      throw Error("assert");
    }
  }
}

function btoaSuccess() {
  const text = "hello world";
  const encoded = btoa(text);
  assert(encoded === "aGVsbG8gd29ybGQ=");
}

function atobSuccess() {
  const encoded = "aGVsbG8gd29ybGQ=";
  const decoded = atob(encoded);
  assert(decoded === "hello world");
}

function atobWithAsciiWhitespace() {
  const encodedList = [
    " aGVsbG8gd29ybGQ=",
    "  aGVsbG8gd29ybGQ=",
    "aGVsbG8gd29ybGQ= ",
    "aGVsbG8gd29ybGQ=\n",
    "aGVsbG\t8gd29ybGQ=",
    `aGVsbG\t8g
                d29ybGQ=`,
  ];

  for (const encoded of encodedList) {
    const decoded = atob(encoded);
    assert(decoded === "hello world");
  }
}

function atobThrows() {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ==");
  } catch (e) {
    threw = true;
  }
  assert(threw);
}

function atobThrows2() {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ===");
  } catch (e) {
    threw = true;
  }
  assert(threw);
}

function btoaFailed() {
  let threw = false;
  const text = "你好";
  try {
    btoa(text);
  } catch (e) {
    assert(e instanceof TypeError);
    threw = true;
  }
  assert(threw);
}

function textDecoder2() {
  // deno-fmt-ignore
  const fixture = new Uint8Array([
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
  const decoder = new TextDecoder();
  assert(decoder.decode(fixture) === "𝓽𝓮𝔁𝓽");
}

function textDecoderIgnoreBOM() {
  // deno-fmt-ignore
  const fixture = new Uint8Array([
    0xef, 0xbb, 0xbf,
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
  const decoder = new TextDecoder("utf-8", { ignoreBOM: true });
  assert(decoder.decode(fixture) === "𝓽𝓮𝔁𝓽");
}

function textDecoderNotBOM() {
  // deno-fmt-ignore
  const fixture = new Uint8Array([
    0xef, 0xbb, 0x89,
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
  const decoder = new TextDecoder("utf-8", { ignoreBOM: true });
  assert(decoder.decode(fixture) === "ﻉ𝓽𝓮𝔁𝓽");
}

function textDecoderASCII() {
  const fixture = new Uint8Array([0x89, 0x95, 0x9f, 0xbf]);
  const decoder = new TextDecoder("ascii");
  assert(decoder.decode(fixture) === "‰•Ÿ¿");
}

function textDecoderErrorEncoding() {
  let didThrow = false;
  try {
    new TextDecoder("foo");
  } catch (e) {
    didThrow = true;
    assert(e.message === "The encoding label provided ('foo') is invalid.");
  }
  assert(didThrow);
}

function textEncoder() {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  // deno-fmt-ignore
  assertArrayEquals(Array.from(encoder.encode(fixture)), [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
}

function textEncodeInto() {
  const fixture = "text";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(5);
  const result = encoder.encodeInto(fixture, bytes);
  assert(result.read === 4);
  assert(result.written === 4);
  // deno-fmt-ignore
  assertArrayEquals(Array.from(bytes), [
    0x74, 0x65, 0x78, 0x74, 0x00,
  ]);
}

function textEncodeInto2() {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(17);
  const result = encoder.encodeInto(fixture, bytes);
  assert(result.read === 8);
  assert(result.written === 16);
  // deno-fmt-ignore
  assertArrayEquals(Array.from(bytes), [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd, 0x00,
  ]);
}

function textEncodeInto3() {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(5);
  const result = encoder.encodeInto(fixture, bytes);
  assert(result.read === 2);
  assert(result.written === 4);
  // deno-fmt-ignore
  assertArrayEquals(Array.from(bytes), [
    0xf0, 0x9d, 0x93, 0xbd, 0x00,
  ]);
}

function textDecoderSharedUint8Array() {
  const ab = new SharedArrayBuffer(6);
  const dataView = new DataView(ab);
  const charCodeA = "A".charCodeAt(0);
  for (let i = 0; i < ab.byteLength; i++) {
    dataView.setUint8(i, charCodeA + i);
  }
  const ui8 = new Uint8Array(ab);
  const decoder = new TextDecoder();
  const actual = decoder.decode(ui8);
  assert(actual === "ABCDEF");
}

function textDecoderSharedInt32Array() {
  const ab = new SharedArrayBuffer(8);
  const dataView = new DataView(ab);
  const charCodeA = "A".charCodeAt(0);
  for (let i = 0; i < ab.byteLength; i++) {
    dataView.setUint8(i, charCodeA + i);
  }
  const i32 = new Int32Array(ab);
  const decoder = new TextDecoder();
  const actual = decoder.decode(i32);
  assert(actual === "ABCDEFGH");
}

function toStringShouldBeWebCompatibility() {
  const encoder = new TextEncoder();
  assert(encoder.toString() === "[object TextEncoder]");

  const decoder = new TextDecoder();
  assert(decoder.toString() === "[object TextDecoder]");
}

function main() {
  btoaSuccess();
  atobSuccess();
  atobWithAsciiWhitespace();
  atobThrows();
  atobThrows2();
  btoaFailed();
  textDecoder2();
  textDecoderIgnoreBOM();
  textDecoderNotBOM();
  textDecoderASCII();
  textDecoderErrorEncoding();
  textEncoder();
  textEncodeInto();
  textEncodeInto2();
  textEncodeInto3();
  textDecoderSharedUint8Array();
  textDecoderSharedInt32Array();
  toStringShouldBeWebCompatibility();
}

main();
