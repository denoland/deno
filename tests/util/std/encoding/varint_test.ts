// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2020 Keith Cirkel. All rights reserved. MIT license.
// This implementation is a port of https://deno.land/x/varint@v2.0.0 by @keithamus

import { assertEquals, assertThrows } from "../assert/mod.ts";
import {
  decode,
  decode32,
  encode,
  MaxUInt64,
  MaxVarIntLen64,
} from "./varint.ts";

function encodeDecode(i: number | bigint) {
  const [buf, n] = encode(i, new Uint8Array(MaxVarIntLen64));
  const fn = (typeof i === "bigint") ? decode : decode32;
  const [j, m] = fn(buf);
  assertEquals(i, j, `${fn.name}(encode(${i})): ${i} !== ${j}`);
  assertEquals(n, m, `${fn.name}(encode(${i})): buffer lengths ${n} !== ${m}`);
}

Deno.test("VarInt decode manual", () => {
  assertEquals(decode(Uint8Array.of(172, 2)), [300n, 2]);
});
Deno.test("VarInt decode max size", () => {
  assertEquals(
    decode(Uint8Array.of(255, 255, 255, 255, 255, 255, 255, 255, 255, 1)),
    [18446744073709551615n, 10],
  );
});
Deno.test("VarInt decode overflow", () => {
  assertThrows(
    () => decode(Uint8Array.of(255, 255, 255, 255, 255, 255, 255, 255, 255, 2)),
    RangeError,
  );
});
Deno.test("VarInt decode with offset", () => {
  assertEquals(
    decode(
      Uint8Array.of(
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        1,
      ),
      4,
    ),
    [18446744073709551615n, 14],
  );
});
Deno.test("VarInt decode32 manual", () => {
  assertEquals(decode32(Uint8Array.of(172, 2)), [300, 2]);
});
Deno.test("VarInt decode32 max size", () => {
  assertEquals(
    decode32(Uint8Array.of(255, 255, 255, 255, 15, 0, 0, 0, 0, 0)),
    [4294967295, 5],
  );
});
Deno.test("VarInt decode32 overflow", () => {
  assertThrows(
    () =>
      decode32(Uint8Array.of(255, 255, 255, 255, 255, 255, 255, 255, 15, 0)),
    RangeError,
  );
});
Deno.test("VarInt decode32 with offset", () => {
  assertEquals(
    decode32(Uint8Array.of(255, 255, 255, 255, 255, 255, 255, 255, 15, 0), 4),
    [4294967295, 9],
  );
});
Deno.test("VarInt encode manual", () => {
  assertEquals(encode(300, new Uint8Array(2)), [Uint8Array.of(172, 2), 2]);
  assertEquals(
    encode(4294967295),
    [Uint8Array.of(255, 255, 255, 255, 15), 5],
  );
  assertEquals(
    encode(18446744073709551615n),
    [Uint8Array.of(255, 255, 255, 255, 255, 255, 255, 255, 255, 1), 10],
  );
});
Deno.test("VarInt encode overflow with negative", () => {
  assertThrows(() => encode(-1), RangeError);
});
Deno.test("VarInt encode with offset", () => {
  let uint = new Uint8Array(3);
  assertEquals(
    encode(300, uint, 1),
    [Uint8Array.of(172, 2), 3],
  );
  assertEquals(uint, Uint8Array.of(0, 172, 2));
  uint = new Uint8Array(MaxVarIntLen64);
  uint[0] = uint[1] = uint[2] = 12;
  assertEquals(
    encode(4294967295, uint, 3),
    [Uint8Array.of(255, 255, 255, 255, 15), 8],
  );
  assertEquals(uint, Uint8Array.of(12, 12, 12, 255, 255, 255, 255, 15, 0, 0));
});
Deno.test("VarInt encode<->decode", () => {
  for (
    const i of [
      0n,
      1n,
      2n,
      10n,
      20n,
      63n,
      64n,
      65n,
      127n,
      128n,
      129n,
      255n,
      256n,
      257n,
      300n,
      18446744073709551615n,
    ]
  ) {
    encodeDecode(i);
  }
  for (let i = 0x7n; i < MaxUInt64; i <<= 1n) {
    encodeDecode(i);
  }
});
Deno.test("VarInt encode<->decode32", () => {
  for (
    const i of [
      0,
      1,
      2,
      10,
      20,
      63,
      64,
      65,
      127,
      128,
      129,
      255,
      256,
      257,
      300,
      4294967295,
    ]
  ) {
    encodeDecode(i);
  }
  for (let i = 0x7; i > 0; i <<= 1) {
    encodeDecode(i);
  }
});
