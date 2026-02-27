// Copyright 2018-2025 the Deno authors. MIT license.
import { assert, assertArrayEquals, assertEquals, test } from "checkin:testing";

test(function testEmptyEncode() {
  const empty = Deno.core.encode("");
  assertEquals(empty.length, 0);
});

test(function testEmptyDecode() {
  const emptyBuf = Deno.core.decode(new Uint8Array(0));
  assertEquals(emptyBuf, "");
});

test(function testFixture1() {
  // deno-fmt-ignore
  const fixture1 = [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ];
  assertArrayEquals(
    Array.from(Deno.core.encode("𝓽𝓮𝔁𝓽")),
    fixture1,
  );
  assertEquals(Deno.core.decode(new Uint8Array(fixture1)), "𝓽𝓮𝔁𝓽");
});

test(function testFixture2() {
  // deno-fmt-ignore
  const fixture2 = [
    72, 101, 108, 108,
    111, 32, 239, 191,
    189, 239, 191, 189,
    32, 87, 111, 114,
    108, 100
  ];
  assertArrayEquals(
    Array.from(Deno.core.encode("Hello \udc12\ud834 World")),
    fixture2,
  );
  assertEquals(
    Deno.core.decode(new Uint8Array(fixture2)),
    "Hello �� World",
  );
});

test(function testStringTooLarge() {
  // See https://github.com/denoland/deno/issues/6649
  let thrown = false;
  try {
    Deno.core.decode(new Uint8Array(2 ** 29));
  } catch (e) {
    thrown = true;
    assert(e instanceof RangeError);
    assertEquals(e.message, "string too long");
  }
  assert(thrown);
});

test(function binaryEncode() {
  function asBinaryString(bytes: Uint8Array): string {
    return Array.from(bytes).map(
      (v: number) => String.fromCodePoint(v),
    ).join("");
  }

  function decodeBinary(binaryString: string) {
    const chars: string[] = Array.from(binaryString);
    return chars.map((v: string): number | undefined => v.codePointAt(0));
  }

  // invalid utf-8 code points
  const invalid = new Uint8Array([0xC0]);
  assertEquals(
    Deno.core.encodeBinaryString(invalid),
    asBinaryString(invalid),
  );

  const invalid2 = new Uint8Array([0xC1]);
  assertEquals(
    Deno.core.encodeBinaryString(invalid2),
    asBinaryString(invalid2),
  );

  for (let i = 0, j = 255; i <= 255; i++, j--) {
    const bytes = new Uint8Array([i, j]);
    const binaryString = Deno.core.encodeBinaryString(bytes);
    assertEquals(
      binaryString,
      asBinaryString(bytes),
    );
    assertArrayEquals(Array.from(bytes), decodeBinary(binaryString));
  }
});
