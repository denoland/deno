// Ported from Go
// https://github.com/golang/go/blob/go1.12.5/src/encoding/hex/hex.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../testing/asserts.ts";

import {
  encodedLen,
  encode,
  encodeToString,
  decodedLen,
  decode,
  decodeString,
  errLength,
  errInvalidByte,
} from "./hex.ts";

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
  ["", "", undefined],
  ["0", "", errLength()],
  ["zd4aa", "", errInvalidByte(toByte("z"))],
  ["d4aaz", "\xd4\xaa", errInvalidByte(toByte("z"))],
  ["30313", "01", errLength()],
  ["0g", "", errInvalidByte(new TextEncoder().encode("g")[0])],
  ["00gg", "\x00", errInvalidByte(new TextEncoder().encode("g")[0])],
  ["0\x01", "", errInvalidByte(new TextEncoder().encode("\x01")[0])],
  ["ffeed", "\xff\xee", errLength()],
];

Deno.test({
  name: "[encoding.hex] encodedLen",
  fn(): void {
    assertEquals(encodedLen(0), 0);
    assertEquals(encodedLen(1), 2);
    assertEquals(encodedLen(2), 4);
    assertEquals(encodedLen(3), 6);
    assertEquals(encodedLen(4), 8);
  },
});

Deno.test({
  name: "[encoding.hex] encode",
  fn(): void {
    {
      const srcStr = "abc";
      const src = new TextEncoder().encode(srcStr);
      const dest = new Uint8Array(encodedLen(src.length));
      const int = encode(dest, src);
      assertEquals(src, new Uint8Array([97, 98, 99]));
      assertEquals(int, 6);
    }

    {
      const srcStr = "abc";
      const src = new TextEncoder().encode(srcStr);
      const dest = new Uint8Array(2); // out of index
      assertThrows(
        (): void => {
          encode(dest, src);
        },
        Error,
        "Out of index."
      );
    }

    for (const [enc, dec] of testCases) {
      const dest = new Uint8Array(encodedLen(dec.length));
      const src = new Uint8Array(dec as number[]);
      const n = encode(dest, src);
      assertEquals(dest.length, n);
      assertEquals(new TextDecoder().decode(dest), enc);
    }
  },
});

Deno.test({
  name: "[encoding.hex] encodeToString",
  fn(): void {
    for (const [enc, dec] of testCases) {
      assertEquals(encodeToString(new Uint8Array(dec as number[])), enc);
    }
  },
});

Deno.test({
  name: "[encoding.hex] decodedLen",
  fn(): void {
    assertEquals(decodedLen(0), 0);
    assertEquals(decodedLen(2), 1);
    assertEquals(decodedLen(4), 2);
    assertEquals(decodedLen(6), 3);
    assertEquals(decodedLen(8), 4);
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
      const dest = new Uint8Array(decodedLen(enc.length));
      const src = new TextEncoder().encode(enc as string);
      const [, err] = decode(dest, src);
      assertEquals(err, undefined);
      assertEquals(Array.from(dest), Array.from(dec as number[]));
    }
  },
});

Deno.test({
  name: "[encoding.hex] decodeString",
  fn(): void {
    for (const [enc, dec] of testCases) {
      const dst = decodeString(enc as string);

      assertEquals(dec, Array.from(dst));
    }
  },
});

Deno.test({
  name: "[encoding.hex] decode error",
  fn(): void {
    for (const [input, output, expectedErr] of errCases) {
      const out = new Uint8Array((input as string).length + 10);
      const [n, err] = decode(out, new TextEncoder().encode(input as string));
      assertEquals(
        new TextDecoder("ascii").decode(out.slice(0, n)),
        output as string
      );
      assertEquals(err, expectedErr);
    }
  },
});

Deno.test({
  name: "[encoding.hex] decodeString error",
  fn(): void {
    for (const [input, output, expectedErr] of errCases) {
      if (expectedErr) {
        assertThrows(
          (): void => {
            decodeString(input as string);
          },
          Error,
          (expectedErr as Error).message
        );
      } else {
        const out = decodeString(input as string);
        assertEquals(new TextDecoder("ascii").decode(out), output as string);
      }
    }
  },
});
