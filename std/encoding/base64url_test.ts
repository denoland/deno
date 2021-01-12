// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../testing/asserts.ts";
import { decode, encode } from "./base64url.ts";

const testsetString = [
  ["", ""],
  ["ß", "w58"],
  ["f", "Zg"],
  ["fo", "Zm8"],
  ["foo", "Zm9v"],
  ["foob", "Zm9vYg"],
  ["fooba", "Zm9vYmE"],
  ["foobar", "Zm9vYmFy"],
  [">?>d?ß", "Pj8-ZD_Dnw"],
];

const testsetBinary = testsetString.map(([str, b64]) => [
  new TextEncoder().encode(str),
  b64,
]) as Array<[Uint8Array, string]>;

Deno.test("[encoding/base64url] testBase64urlEncodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encode(input), output);
  }
});

Deno.test("[decoding/base64url] testBase64urlDecodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(decode(output), input);
  }
});
