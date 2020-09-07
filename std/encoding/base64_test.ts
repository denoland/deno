// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { encode, decode, decodeString } from "./base64.ts";

const testsetString = [
  ["", ""],
  ["f", "Zg=="],
  ["fo", "Zm8="],
  ["foo", "Zm9v"],
  ["foob", "Zm9vYg=="],
  ["fooba", "Zm9vYmE="],
  ["foobar", "Zm9vYmFy"],
];

const testsetBinary = [
  [new TextEncoder().encode("\x00"), "AA=="],
  [new TextEncoder().encode("\x00\x00"), "AAA="],
  [new TextEncoder().encode("\x00\x00\x00"), "AAAA"],
  [new TextEncoder().encode("\x00\x00\x00\x00"), "AAAAAA=="],
];

Deno.test("[encoding/base64] testBase64EncodeString", () => {
  for (const [input, output] of testsetString) {
    assertEquals(encode(input), output);
  }
});

Deno.test("[encoding/base64] testBase64DecodeString", () => {
  for (const [input, output] of testsetString) {
    assertEquals(decodeString(output), input);
  }
});

Deno.test("[encoding/base64] testBase64EncodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encode(input), output);
  }
});

Deno.test("[encoding/base64] testBase64DecodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    const outputBinary = new Uint8Array(decode(output as string));
    assertEquals(outputBinary, input as Uint8Array);
  }
});
