// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, unitTest } from "./test_util.ts";

unitTest(function btoaSuccess() {
  const text = "hello world";
  const encoded = btoa(text);
  assertEquals(encoded, "aGVsbG8gd29ybGQ=");
});

unitTest(function atobSuccess() {
  const encoded = "aGVsbG8gd29ybGQ=";
  const decoded = atob(encoded);
  assertEquals(decoded, "hello world");
});

unitTest(function atobWithAsciiWhitespace() {
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
    assertEquals(decoded, "hello world");
  }
});

unitTest(function atobThrows() {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ==");
  } catch (_e) {
    threw = true;
  }
  assert(threw);
});

unitTest(function atobThrows2() {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ===");
  } catch (_e) {
    threw = true;
  }
  assert(threw);
});

unitTest(function atobThrows3() {
  let threw = false;
  try {
    atob("foobar!!");
  } catch (e) {
    if (
      e instanceof DOMException &&
      e.toString().startsWith("InvalidCharacterError:")
    ) {
      threw = true;
    }
  }
  assert(threw);
});

unitTest(function btoaFailed() {
  const text = "你好";
  assertThrows(() => {
    btoa(text);
  }, DOMException);
});

unitTest(function textDecoder2() {
  // deno-fmt-ignore
  const fixture = new Uint8Array([
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
  const decoder = new TextDecoder();
  assertEquals(decoder.decode(fixture), "𝓽𝓮𝔁𝓽");
});

// ignoreBOM is tested through WPT

unitTest(function textDecoderASCII() {
  const fixture = new Uint8Array([0x89, 0x95, 0x9f, 0xbf]);
  const decoder = new TextDecoder("ascii");
  assertEquals(decoder.decode(fixture), "‰•Ÿ¿");
});

unitTest(function textDecoderErrorEncoding() {
  let didThrow = false;
  try {
    new TextDecoder("Foo");
  } catch (e) {
    didThrow = true;
    assertEquals(e.message, "The encoding label provided ('Foo') is invalid.");
  }
  assert(didThrow);
});

unitTest(function textEncoder() {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  // deno-fmt-ignore
  assertEquals(Array.from(encoder.encode(fixture)), [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ]);
});

unitTest(function textEncodeInto() {
  const fixture = "text";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(5);
  const result = encoder.encodeInto(fixture, bytes);
  assertEquals(result.read, 4);
  assertEquals(result.written, 4);
  // deno-fmt-ignore
  assertEquals(Array.from(bytes), [
    0x74, 0x65, 0x78, 0x74, 0x00,
  ]);
});

unitTest(function textEncodeInto2() {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(17);
  const result = encoder.encodeInto(fixture, bytes);
  assertEquals(result.read, 8);
  assertEquals(result.written, 16);
  // deno-fmt-ignore
  assertEquals(Array.from(bytes), [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd, 0x00,
  ]);
});

unitTest(function textEncodeInto3() {
  const fixture = "𝓽𝓮𝔁𝓽";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(5);
  const result = encoder.encodeInto(fixture, bytes);
  assertEquals(result.read, 2);
  assertEquals(result.written, 4);
  // deno-fmt-ignore
  assertEquals(Array.from(bytes), [
    0xf0, 0x9d, 0x93, 0xbd, 0x00,
  ]);
});

unitTest(function loneSurrogateEncodeInto() {
  const fixture = "lone𝄞\ud888surrogate";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(20);
  const result = encoder.encodeInto(fixture, bytes);
  assertEquals(result.read, 16);
  assertEquals(result.written, 20);
  // deno-fmt-ignore
  assertEquals(Array.from(bytes), [
    0x6c, 0x6f, 0x6e, 0x65,
    0xf0, 0x9d, 0x84, 0x9e,
    0xef, 0xbf, 0xbd, 0x73,
    0x75, 0x72, 0x72, 0x6f,
    0x67, 0x61, 0x74, 0x65
  ]);
});

unitTest(function loneSurrogateEncodeInto2() {
  const fixture = "\ud800";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(3);
  const result = encoder.encodeInto(fixture, bytes);
  assertEquals(result.read, 1);
  assertEquals(result.written, 3);
  // deno-fmt-ignore
  assertEquals(Array.from(bytes), [
    0xef, 0xbf, 0xbd
  ]);
});

unitTest(function loneSurrogateEncodeInto3() {
  const fixture = "\udc00";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(3);
  const result = encoder.encodeInto(fixture, bytes);
  assertEquals(result.read, 1);
  assertEquals(result.written, 3);
  // deno-fmt-ignore
  assertEquals(Array.from(bytes), [
    0xef, 0xbf, 0xbd
  ]);
});

unitTest(function swappedSurrogatePairEncodeInto4() {
  const fixture = "\udc00\ud800";
  const encoder = new TextEncoder();
  const bytes = new Uint8Array(8);
  const result = encoder.encodeInto(fixture, bytes);
  assertEquals(result.read, 2);
  assertEquals(result.written, 6);
  // deno-fmt-ignore
  assertEquals(Array.from(bytes), [
    0xef, 0xbf, 0xbd, 0xef, 0xbf, 0xbd, 0x00, 0x00
  ]);
});

unitTest(function textDecoderSharedUint8Array() {
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

unitTest(function textDecoderSharedInt32Array() {
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

unitTest(function toStringShouldBeWebCompatibility() {
  const encoder = new TextEncoder();
  assertEquals(encoder.toString(), "[object TextEncoder]");

  const decoder = new TextDecoder();
  assertEquals(decoder.toString(), "[object TextDecoder]");
});
unitTest(function textEncoderShouldCoerceToString() {
  const encoder = new TextEncoder();
  const fixutreText = "text";
  const fixture = {
    toString() {
      return fixutreText;
    },
  };

  const bytes = encoder.encode(fixture as unknown as string);
  const decoder = new TextDecoder();
  const decoded = decoder.decode(bytes);
  assertEquals(decoded, fixutreText);
});
