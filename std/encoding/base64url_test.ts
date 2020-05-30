// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { encode, decode } from "./base64url.ts";

const testsetString = [
  ["", ""],
  ["f", "Zg"],
  ["fo", "Zm8"],
  ["foo", "Zm9v"],
  ["foob", "Zm9vYg"],
  ["fooba", "Zm9vYmE"],
  ["foobar", "Zm9vYmFy"],
  [">?>d?ß", "Pj8-ZD_f"],
];

const testsetBinary = [
  [new TextEncoder().encode("\x00"), "AA"],
  [new TextEncoder().encode("\x00\x00"), "AAA"],
  [new TextEncoder().encode("\x00\x00\x00"), "AAAA"],
  [new TextEncoder().encode("\x00\x00\x00\x00"), "AAAAAA"],
];

test("[encoding/base64url] testBase64urlEncodeString", () => {
  for (const [input, output] of testsetString) {
    assertEquals(encode(input), output);
  }
});

test("[encoding/base64url] testBase64urlEncodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encode(input), output);
  }
});

test("[encoding/base64ur] testBase64urDecodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    const outputBinary = new Uint8Array(decode(output as string));
    assertEquals(outputBinary, input as Uint8Array);
  }
});
