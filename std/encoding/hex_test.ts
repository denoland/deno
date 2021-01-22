// Ported from Go
// https://github.com/golang/go/blob/go1.12.5/src/encoding/hex/hex.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../testing/asserts.ts";

import { decode, encode } from "./hex.ts";

function toByte(s: string): number {
  return new TextEncoder().encode(s)[0];
}

const testCases = [
  // encoded(hex) / decoded(Uint8Array)
  ["", []],
  ["0001020304050607", [0, 1, 2, 3, 4, 5, 6, 7]],
  ["08090a0b0c0d0e0f", [8, 9, 10, 11, 12, 13, 14, 15]],
  ["f0f1f2f3f4f5f6f7", [0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7]],
  ["f8f9fafbfcfdfeff", [0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff]],
  ["67", Array.from(new TextEncoder().encode("g"))],
  ["e3a1", [0xe3, 0xa1]],
];

const errCases = [
  // encoded(hex) / error
  ["0", _errLength()],
  ["zd4aa", _errInvalidByte(toByte("z"))],
  ["d4aaz", _errInvalidByte(toByte("z"))],
  ["30313", _errLength()],
  ["0g", _errInvalidByte(new TextEncoder().encode("g")[0])],
  ["00gg", _errInvalidByte(new TextEncoder().encode("g")[0])],
  ["0\x01", _errInvalidByte(new TextEncoder().encode("\x01")[0])],
  ["ffeed", _errLength()],
];

Deno.test({
  name: "[encoding.hex] encode",
  fn(): void {
    {
      const srcStr = "abc";
      const src = new TextEncoder().encode(srcStr);
      const dest = encode(src);
      assertEquals(src, new Uint8Array([97, 98, 99]));
      assertEquals(dest.length, 6);
    }

    for (const [enc, dec] of testCases) {
      const src = new Uint8Array(dec as number[]);
      const dest = encode(src);
      assertEquals(dest.length, src.length * 2);
      assertEquals(new TextDecoder().decode(dest), enc);
    }
  },
});

Deno.test({
  name: "[encoding.hex] decode",
  fn(): void {
    // Case for decoding uppercase hex characters, since
    // Encode always uses lowercase.
    const extraTestcase = [
      ["F8F9FAFBFCFDFEFF", [0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff]],
    ];

    const cases = testCases.concat(extraTestcase);

    for (const [enc, dec] of cases) {
      const src = new TextEncoder().encode(enc as string);
      const dest = decode(src);
      assertEquals(Array.from(dest), Array.from(dec as number[]));
    }
  },
});

Deno.test({
  name: "[encoding.hex] decode error",
  fn(): void {
    for (const [input, expectedErr] of errCases) {
      assertThrows(
        () => decode(new TextEncoder().encode(input as string)),
        Error,
        (expectedErr as Error).message,
      );
    }
  },
});

function _errLength() {
  return new TypeError("odd length of hex string");
}
function _errInvalidByte(byte: number) {
  new TypeError(
    `received invalid byte: ${
      new TextDecoder().decode(new Uint8Array([byte]))
    }`,
  );
}
