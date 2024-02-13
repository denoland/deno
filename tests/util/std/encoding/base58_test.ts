// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../assert/mod.ts";
import { decodeBase58, encodeBase58 } from "./base58.ts";

const testSetString = [
  ["", ""],
  ["f", "2m"],
  ["ß", "FtS"],
  ["fo", "8o8"],
  ["foo", "bQbp"],
  ["foob", "3csAg9"],
  ["fooba", "CZJRhmz"],
  ["foobar", "t1Zv2yaZ"],
  ["Hello World!", "2NEpo7TZRRrLZSi2U"],
  [new Uint8Array([0, 0, 0, 40, 127, 180, 205]), "111233QC4"],
  [new Uint8Array([10, 0, 10]), "4MpV"],
  [
    // deno-fmt-ignore
    new Uint8Array([
        2, 212, 53, 147, 199,  21, 253, 211,
       28,  97, 20,  26, 189,   4, 169, 159,
      214, 130, 44, 133,  88, 133,  76, 205,
      227, 154, 86, 132, 231, 165, 109, 162,
      125, 137, 84
    ]),
    "HNZata7iMYWmk5RvZRTiAsSDhV8366zq2YGb3tLH5Upf74F",
  ],
];

const testSetBinary = testSetString.map(([data, b58]) => {
  if (typeof data === "string") {
    return [
      new TextEncoder().encode(data),
      b58,
    ];
  }

  return [data, b58];
}) as Array<[Uint8Array, string]>;

Deno.test("[encoding/base58] testBase58EncodeString", () => {
  for (const [input, output] of testSetString) {
    assertEquals(encodeBase58(input), output);
  }
});

Deno.test("[encoding/base58] testBase58EncodeBinary", () => {
  for (const [input, output] of testSetBinary) {
    assertEquals(encodeBase58(input), output);
  }
});

Deno.test("[encoding/base58] testBase58EncodeBinaryBuffer", () => {
  for (const [input, output] of testSetBinary) {
    assertEquals(encodeBase58(input.buffer), output);
  }
});

Deno.test("[encoding/base58] testBase58DecodeBinary", () => {
  for (const [input, output] of testSetBinary) {
    const outputBinary = decodeBase58(output);
    assertEquals(outputBinary, input);
  }
});

Deno.test("[encoding/base58] testBase58DecodeError", () => {
  assertThrows(
    () => decodeBase58("+2NEpo7TZRRrLZSi2U"),
    `Invalid base58 char at index 0 with value +`,
  );
});
