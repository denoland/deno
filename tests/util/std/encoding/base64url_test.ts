// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../assert/mod.ts";
import { decodeBase64Url, encodeBase64Url } from "./base64url.ts";

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

const testsetInvalid = [
  "Pj8/ZD+Dnw",
  "PDw/Pz8+Pg",
  "Pj8/ZD+Dnw==",
  "PDw/Pz8+Pg==",
];

Deno.test("[encoding/base64url] testBase64urlEncodeString", () => {
  for (const [input, output] of testsetString) {
    assertEquals(encodeBase64Url(input), output);
  }
});

Deno.test("[encoding/base64url] testBase64urlEncodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(encodeBase64Url(input), output);
  }
});

Deno.test("[decoding/base64url] testBase64urlDecodeBinary", () => {
  for (const [input, output] of testsetBinary) {
    assertEquals(decodeBase64Url(output), input);
  }
});

Deno.test("[decoding/base64url] base64url.decode throws on invalid input", () => {
  for (const invalidb64url of testsetInvalid) {
    assertThrows(
      () => decodeBase64Url(invalidb64url),
      TypeError,
      "invalid character",
    );
  }
});
