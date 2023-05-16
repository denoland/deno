// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { Buffer } from "node:buffer";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

const bufferLen12 = Buffer.from("Hello, World!");
const bufferLen11 = Buffer.from("Hello, World");

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

for (const encoding of BUFFER_ENCODINGS) {
  Deno.test({
    name: "Buffer.from with encoding " + encoding,
    fn: () => {
      // @ts-ignore: BufferEncoding is a string but TS complains
      assertEquals(
        Buffer.from(bufferLen12.toString(encoding), encoding),
        bufferLen12,
      );
      // @ts-ignore: BufferEncoding is a string but TS complains
      assertEquals(
        Buffer.from(bufferLen11.toString(encoding), encoding),
        bufferLen11,
      );
    },
  });
}
