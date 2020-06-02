// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "31d6cfe0d16ae931b73c59d7e0c089c0"],
  ["abc", "a448017aaf21d8525fc10ae87aa6729d"],
  ["deno", "594749a3bef632d12ab7067469aa8aed"],
  [
    "The quick brown fox jumps over the lazy dog",
    "1bee69a46ba811185c194762abaeae90",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "d5f9a9e9257077a5f08b0b92f348b0ad",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "52f5076fabd22680234a3fa9f9dc5732",
  ],
  [millionAs, "bbce80cc6bb65e5c6745e30d4eeca9a4"],
];

const testSetBase64 = [
  ["", "MdbP4NFq6TG3PFnX4MCJwA=="],
  ["abc", "pEgBeq8h2FJfwQroeqZynQ=="],
  ["deno", "WUdJo772MtEqtwZ0aaqK7Q=="],
  ["The quick brown fox jumps over the lazy dog", "G+5ppGuoERhcGUdiq66ukA=="],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "1fmp6SVwd6XwiwuS80iwrQ==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "UvUHb6vSJoAjSj+p+dxXMg==",
  ],
  [millionAs, "u86AzGu2XlxnReMNTuyppA=="],
];

test("[hash/md4] testMd4Hex", () => {
  for (const [input, output] of testSetHex) {
    const md4 = createHash("md4");
    assertEquals(md4.update(input).toString(), output);
    md4.dispose();
  }
});

test("[hash/md4] testMd4Base64", () => {
  for (const [input, output] of testSetBase64) {
    const md4 = createHash("md4");
    assertEquals(md4.update(input).toString("base64"), output);
    md4.dispose();
  }
});
