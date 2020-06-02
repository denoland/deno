// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  [
    "",
    "38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b",
  ],
  [
    "abc",
    "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7",
  ],
  [
    "deno",
    "d6a359079da9d9a1c8ecec1d84b85ed9ca198976bfa50953867536d79e8628480f6e63adcb7f6a782de68bf5a1c96349",
  ],
  [
    "The quick brown fox jumps over the lazy dog",
    "ca737f1014a48f4c0b6dd43cb177b0afd9e5169367544c494011e3317dbf9a509cb1e5dc1e85a941bbee3d7f2afbc9b1",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "8a8d9649ea04e993a6ca7135af7e3392cc5fca84f8531cac7aa3feed4eb98f55dcbe0f3284b61c6f35f98b02cc644b4c",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "2e404b9339da795776e510d96930b3be2904c500395b8cb7413334b82d4dec413b4b8113045a05bbbcff846f027423f6",
  ],
  [
    millionAs,
    "9d0e1809716474cb086e834e310a4a1ced149e9c00f248527972cec5704c2a5b07b8b3dc38ecc4ebae97ddd87f3d8985",
  ],
];

const testSetBase64 = [
  ["", "OLBgp1GsljhM2TJ+sbHjaiH9txEUvgdDTAzHv2P24donTt6/529l+9Ua0vFImLlb"],
  ["abc", "ywB1P0WjXou1oD1pmsZQBycsMqsO3tFjGotgWkP/W+2AhgcroefMI1i67KE0yCWn"],
  ["deno", "1qNZB52p2aHI7OwdhLhe2coZiXa/pQlThnU2156GKEgPbmOty39qeC3mi/WhyWNJ"],
  [
    "The quick brown fox jumps over the lazy dog",
    "ynN/EBSkj0wLbdQ8sXewr9nlFpNnVExJQBHjMX2/mlCcseXcHoWpQbvuPX8q+8mx",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "io2WSeoE6ZOmynE1r34zksxfyoT4UxyseqP+7U65j1Xcvg8yhLYcbzX5iwLMZEtM",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "LkBLkznaeVd25RDZaTCzvikExQA5W4y3QTM0uC1N7EE7S4ETBFoFu7z/hG8CdCP2",
  ],
  [
    millionAs,
    "nQ4YCXFkdMsIboNOMQpKHO0UnpwA8khSeXLOxXBMKlsHuLPcOOzE666X3dh/PYmF",
  ],
];

test("[hash/sha384] testSha384Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha384 = createHash("sha384");
    assertEquals(sha384.update(input).toString(), output);
    sha384.dispose();
  }
});

test("[hash/sha384] testSha384Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha384 = createHash("sha384");
    assertEquals(sha384.update(input).toString("base64"), output);
    sha384.dispose();
  }
});
