// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f"],
  ["abc", "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7"],
  ["deno", "c34ee73c656a6a6437b70610e261be4412c650acabdb20e26f11f620"],
  [
    "The quick brown fox jumps over the lazy dog",
    "730e109bd7a8a32b1cb9d9a09aa2325d2430587ddbc0c38bad911525",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "d40854fc9caf172067136f2e29e1380b14626bf6f0dd06779f820dcd",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "a88cd5cde6d6fe9136a4e58b49167461ea95d388ca2bdb7afdc3cbf4",
  ],
  [millionAs, "20794655980c91d8bbb4c1ea97618a4bf03f42581948b2ee4ee7ad67"],
];

const testSetBase64 = [
  ["", "0UoCjCo6K8lHYQK7KII0xBWisB+CjqYqxbPkLw=="],
  ["abc", "Iwl9IjQF2CKGQqR3vaJVsyqtvOS9oLP342ydpw=="],
  ["deno", "w07nPGVqamQ3twYQ4mG+RBLGUKyr2yDibxH2IA=="],
  [
    "The quick brown fox jumps over the lazy dog",
    "cw4Qm9eooyscudmgmqIyXSQwWH3bwMOLrZEVJQ==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "1AhU/JyvFyBnE28uKeE4CxRia/bw3QZ3n4INzQ==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "qIzVzebW/pE2pOWLSRZ0YeqV04jKK9t6/cPL9A==",
  ],
  [millionAs, "IHlGVZgMkdi7tMHql2GKS/A/QlgZSLLuTuetZw=="],
];

test("[hash/sha224] testSha224Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha224 = createHash("sha224");
    assertEquals(sha224.update(input).toString(), output);
    sha224.dispose();
  }
});

test("[hash/sha224] testSha224Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha224 = createHash("sha224");
    assertEquals(sha224.update(input).toString("base64"), output);
    sha224.dispose();
  }
});
