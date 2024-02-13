// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../assert/mod.ts";
import { decode } from "./decode.ts";

Deno.test("positive fixint", () => {
  for (let i = 0; i <= 0x7f; i++) {
    assertEquals(decode(Uint8Array.of(i)), i);
  }
});

Deno.test("fixmap", () => {
  const map = { "a": 2, "b": 3 };
  const encodedMap = [0b1010_0001, 97, 2, 0b1010_0001, 98, 3];

  assertEquals(decode(Uint8Array.of(0b10000000 | 2, ...encodedMap)), map);
});

Deno.test("fixarray", () => {
  const array = [0, 1, 2, 3, 4, 5, 6];

  assertEquals(
    decode(Uint8Array.of(0b10010000 | array.length, ...array)),
    array,
  );
});

Deno.test("fixstr", () => {
  const str = "hello world!";
  const encoded = new TextEncoder().encode(str);

  assertEquals(
    decode(Uint8Array.of(0xA0 | encoded.length, ...encoded)),
    str,
  );
});

Deno.test("nil, (never used), false, true", () => {
  assertEquals(decode(Uint8Array.of(0xc0)), null); // nil
  assertThrows(() => decode(Uint8Array.of(0xc1))); // (never used)
  assertEquals(decode(Uint8Array.of(0xc2)), false); // false
  assertEquals(decode(Uint8Array.of(0xc3)), true); // true
});

Deno.test("bin 8, bin 16, bin 32", () => {
  const arr = Uint8Array.of(0, 1, 2, 3, 4, 5, 6, 7);
  assertEquals(decode(Uint8Array.of(0xc4, arr.length, ...arr)), arr);
  assertEquals(decode(Uint8Array.of(0xc5, 0, arr.length, ...arr)), arr);
  assertEquals(
    decode(Uint8Array.of(0xc6, 0, 0, 0, arr.length, ...arr)),
    arr,
  );
});

Deno.test("ext 8, ext 16, ext 32", () => {
  assertThrows(() => decode(Uint8Array.of(0xc7)));
  assertThrows(() => decode(Uint8Array.of(0xc8)));
  assertThrows(() => decode(Uint8Array.of(0xc9)));
});

Deno.test("float 32, float 64", () => {
  assertEquals(
    decode(Uint8Array.of(0xca, 0x43, 0xd2, 0x58, 0x52)),
    420.69000244140625,
  );
  assertEquals(
    decode(
      Uint8Array.of(0xcb, 0x40, 0x09, 0x21, 0xFB, 0x54, 0x44, 0x2D, 0x18),
    ),
    3.14159265358979311599796346854,
  );
});

Deno.test("uint8, uint16, uint32, uint64", () => {
  assertEquals(decode(Uint8Array.of(0xcc, 0xff)), 255);
  assertEquals(decode(Uint8Array.of(0xcd, 0xff, 0xff)), 65535);
  assertEquals(
    decode(Uint8Array.of(0xce, 0xff, 0xff, 0xff, 0xff)),
    4294967295,
  );
  assertEquals(
    decode(
      Uint8Array.of(0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff),
    ),
    18446744073709551615n,
  );
});

Deno.test("int8, int16, int32, int64", () => {
  assertEquals(decode(Uint8Array.of(0xd0, 0x80)), -128);
  assertEquals(decode(Uint8Array.of(0xd1, 0x80, 0x00)), -32768);
  assertEquals(
    decode(Uint8Array.of(0xd2, 0x80, 0x00, 0x00, 0x00)),
    -2147483648,
  );
  assertEquals(
    decode(
      Uint8Array.of(0xd3, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00),
    ),
    -9223372036854775808n,
  );
});

Deno.test("fixext 1, fixext 2, fixext 4, fixext 8, fixext 16", () => {
  assertThrows(() => decode(Uint8Array.of(0xd4)));
  assertThrows(() => decode(Uint8Array.of(0xd5)));
  assertThrows(() => decode(Uint8Array.of(0xd6)));
  assertThrows(() => decode(Uint8Array.of(0xd7)));
  assertThrows(() => decode(Uint8Array.of(0xd8)));
});

Deno.test("str 8, str 16, str 32", () => {
  const str = "hello world!";
  const encoded = new TextEncoder().encode(str);

  assertEquals(decode(Uint8Array.of(0xd9, encoded.length, ...encoded)), str);
  assertEquals(
    decode(Uint8Array.of(0xda, 0, encoded.length, ...encoded)),
    str,
  );
  assertEquals(
    decode(Uint8Array.of(0xdb, 0, 0, 0, encoded.length, ...encoded)),
    str,
  );
});

Deno.test("array 16, array 32", () => {
  const array = [0, 1, 2, 3, 4, 5, 6];

  assertEquals(
    decode(Uint8Array.of(0xdc, 0, array.length, ...array)),
    array,
  );
  assertEquals(
    decode(Uint8Array.of(0xdd, 0, 0, 0, array.length, ...array)),
    array,
  );
});

Deno.test("map 16, map 32", () => {
  const map = { "a": 2, "b": 3 };
  const encodedMap = [0b1010_0001, 97, 2, 0b1010_0001, 98, 3];

  assertEquals(decode(Uint8Array.of(0xde, 0, 2, ...encodedMap)), map);
  assertEquals(decode(Uint8Array.of(0xdf, 0, 0, 0, 2, ...encodedMap)), map);
});

Deno.test("negative fixint", () => {
  for (let i = -32; i <= -1; i++) {
    assertEquals(decode(Uint8Array.of(i)), i);
  }
});
