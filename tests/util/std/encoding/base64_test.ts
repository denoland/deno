// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { decodeBase64, encodeBase64 } from "./base64.ts";

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
    assertEquals(encodeBase64(input), output);
  }
});

Deno.test("[encoding/base64] testBase64EncodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encodeBase64(input), output);
  }
});

Deno.test("[encoding/base64] testBase64EncodeBinaryBuffer", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encodeBase64(input.buffer), output);
  }
});

Deno.test("[encoding/base64] testBase64DecodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    const outputBinary = decodeBase64(output);
    assertEquals(outputBinary, input);
  }
});
