// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../testing/asserts.ts";
import { decode, encode } from "./base64.ts";

const testsetString = [
  ["", ""],
  ["ß", "w58="],
  ["f", "Zg=="],
  ["fo", "Zm8="],
  ["foo", "Zm9v"],
  ["foob", "Zm9vYg=="],
  ["fooba", "Zm9vYmE="],
  ["foobar", "Zm9vYmFy"],
];

const testsetBinary = testsetString.map(([str, b64]) => [
  new TextEncoder().encode(str),
  b64,
]) as Array<[Uint8Array, string]>;

Deno.test("[encoding/base64] testBase64EncodeString", () => {
  for (const [input, output] of testsetString) {
    assertEquals(encode(input), output);
  }
});

Deno.test("[encoding/base64] testBase64EncodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encode(input), output);
  }
});

Deno.test("[encoding/base64] testBase64EncodeBinaryBuffer", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encode(input.buffer), output);
  }
});

Deno.test("[encoding/base64] testBase64DecodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    const outputBinary = decode(output);
    assertEquals(outputBinary, input);
  }
});
