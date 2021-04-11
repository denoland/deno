// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "./test_util.ts";

Deno.test("btoaSuccess", function (): void {
  const text = "hello world";
  const encoded = btoa(text);
  assertEquals(encoded, "aGVsbG8gd29ybGQ=");
});

Deno.test("atobSuccess", function (): void {
  const encoded = "aGVsbG8gd29ybGQ=";
  const decoded = atob(encoded);
  assertEquals(decoded, "hello world");
});

Deno.test("atobWithAsciiWhitespace", function (): void {
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

Deno.test("atobThrows", function (): void {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ==");
  } catch (_e) {
    threw = true;
  }
  assert(threw);
});

Deno.test("atobThrows2", function (): void {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ===");
  } catch (_e) {
    threw = true;
  }
  assert(threw);
});

Deno.test("atobThrows3", function (): void {
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

Deno.test("btoaFailed", function (): void {
  const text = "你好";
  assertThrows(() => {
    btoa(text);
  }, DOMException);
});

Deno.test("textDecoder2", function (): void {
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

Deno.test("textDecoderASCII", function (): void {
  const fixture = new Uint8Array([0x89, 0x95, 0x9f, 0xbf]);
  const decoder = new TextDecoder("ascii");
  assertEquals(decoder.decode(fixture), "‰•Ÿ¿");
});

Deno.test("textDecoderErrorEncoding", function (): void {
  let didThrow = false;
  try {
    new TextDecoder("Foo");
  } catch (e) {
    didThrow = true;
    assertEquals(e.message, "The encoding label provided ('Foo') is invalid.");
  }
  assert(didThrow);
});

Deno.test("textEncoder", function (): void {
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

Deno.test("textEncodeInto", function (): void {
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

Deno.test("textEncodeInto2", function (): void {
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

Deno.test("textEncodeInto3", function (): void {
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

Deno.test("textDecoderSharedUint8Array", function (): void {
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

Deno.test("textDecoderSharedInt32Array", function (): void {
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

Deno.test("toStringShouldBeWebCompatibility", function (): void {
  const encoder = new TextEncoder();
  assertEquals(encoder.toString(), "[object TextEncoder]");

  const decoder = new TextDecoder();
  assertEquals(decoder.toString(), "[object TextDecoder]");
});
Deno.test("textEncoderShouldCoerceToString", function (): void {
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
