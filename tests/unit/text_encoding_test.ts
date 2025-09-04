// Copyright 2018-2025 the Deno authors. MIT license.
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrows,
} from "./test_util.ts";

Deno.test(function btoaSuccess() {
  const text = "hello world";
  const encoded = btoa(text);
  assertEquals(encoded, "aGVsbG8gd29ybGQ=");
});

Deno.test(function atobSuccess() {
  const encoded = "aGVsbG8gd29ybGQ=";
  const decoded = atob(encoded);
  assertEquals(decoded, "hello world");
});

Deno.test(function atobWithAsciiWhitespace() {
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

Deno.test(function atobThrows() {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ==");
  } catch (_e) {
    threw = true;
  }
  assert(threw);
});

Deno.test(function atobThrows2() {
  let threw = false;
  try {
    atob("aGVsbG8gd29ybGQ===");
  } catch (_e) {
    threw = true;
  }
  assert(threw);
});

Deno.test(function atobThrows3() {
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

Deno.test(function btoaFailed() {
  const text = "你好";
  assertThrows(() => {
    btoa(text);
  }, DOMException);
});

Deno.test(function textDecoder2() {
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

Deno.test(function textDecoderASCII() {
  const fixture = new Uint8Array([0x89, 0x95, 0x9f, 0xbf]);
  const decoder = new TextDecoder("ascii");
  assertEquals(decoder.decode(fixture), "‰•Ÿ¿");
});

Deno.test(function textDecoderErrorEncoding() {
  let didThrow = false;
  try {
    new TextDecoder("Foo");
  } catch (e) {
    didThrow = true;
    assert(e instanceof Error);
    assertEquals(e.message, "The encoding label provided ('Foo') is invalid.");
  }
  assert(didThrow);
});

Deno.test(function textEncoder() {
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

Deno.test(function textEncodeInto() {
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

Deno.test(function textEncodeInto2() {
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

Deno.test(function textEncodeInto3() {
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

Deno.test(function loneSurrogateEncodeInto() {
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

Deno.test(function loneSurrogateEncodeInto2() {
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

Deno.test(function loneSurrogateEncodeInto3() {
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

Deno.test(function swappedSurrogatePairEncodeInto4() {
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

Deno.test(function textDecoderSharedUint8Array() {
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

Deno.test(function textDecoderSharedInt32Array() {
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

Deno.test(function toStringShouldBeWebCompatibility() {
  const encoder = new TextEncoder();
  assertEquals(encoder.toString(), "[object TextEncoder]");

  const decoder = new TextDecoder();
  assertEquals(decoder.toString(), "[object TextDecoder]");
});

Deno.test(function textEncoderShouldCoerceToString() {
  const encoder = new TextEncoder();
  const fixtureText = "text";
  const fixture = {
    toString() {
      return fixtureText;
    },
  };

  const bytes = encoder.encode(fixture as unknown as string);
  const decoder = new TextDecoder();
  const decoded = decoder.decode(bytes);
  assertEquals(decoded, fixtureText);
});

Deno.test(function binaryEncode() {
  // @ts-ignore: Deno[Deno.internal].core allowed
  const core = Deno[Deno.internal].core;
  function asBinaryString(bytes: Uint8Array): string {
    return Array.from(bytes).map(
      (v: number) => String.fromCodePoint(v),
    ).join("");
  }

  function decodeBinary(binaryString: string) {
    const chars: string[] = Array.from(binaryString);
    return chars.map((v: string): number | undefined => v.codePointAt(0));
  }
  const inputs = [
    "σ😀",
    "Кириллица is Cyrillic",
    "𝓽𝓮𝔁𝓽",
    "lone𝄞\ud888surrogate",
    "\udc00\ud800",
    "\ud800",
  ];
  for (const input of inputs) {
    const bytes = new TextEncoder().encode(input);
    const binaryString = core.encodeBinaryString(bytes);
    assertEquals(
      binaryString,
      asBinaryString(bytes),
    );

    assertEquals(Array.from(bytes), decodeBinary(binaryString));
  }
});

Deno.test(
  { permissions: { read: true } },
  async function textDecoderStreamCleansUpOnCancel() {
    let cancelled = false;
    const readable = new ReadableStream({
      start: (controller) => {
        controller.enqueue(new Uint8Array(12));
      },
      cancel: () => {
        cancelled = true;
      },
    }).pipeThrough(new TextDecoderStream());
    const chunks = [];
    for await (const chunk of readable) {
      chunks.push(chunk);
      // breaking out of the loop prevents normal shutdown at end of async iterator values and triggers the cancel method of the stream instead
      break;
    }
    assertEquals(chunks.length, 1);
    assertEquals(chunks[0].length, 12);
    assertStrictEquals(cancelled, true);
  },
);
