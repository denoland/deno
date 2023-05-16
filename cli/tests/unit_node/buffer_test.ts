// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { Buffer } from 'node:buffer';
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

const buffer_len12 = Buffer.from("Hello, World!");
const buffer_len11 = Buffer.from("Hello, World");

const BUFFER_ENCODINGS = [
  "hex",
  "base64",
  "base64url",
  "ascii",
  "latin1",
  "ucs2",
  "utf8",
  "utf16le",
];

for (let encoding of BUFFER_ENCODINGS) {
  Deno.test({
    name: "Buffer.from with encoding " + encoding,
    fn: () => {
      // @ts-ignore
      assertEquals(Buffer.from(buffer_len12.toString(encoding), encoding), buffer_len12);
      // @ts-ignore
      assertEquals(Buffer.from(buffer_len11.toString(encoding), encoding), buffer_len11);
    },
  });
}
