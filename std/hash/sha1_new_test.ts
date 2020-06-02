// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "da39a3ee5e6b4b0d3255bfef95601890afd80709"],
  ["abc", "a9993e364706816aba3e25717850c26c9cd0d89d"],
  ["deno", "bb3d8e712d9e7ad4af08d4a38f3f52d9683d58eb"],
  [
    "The quick brown fox jumps over the lazy dog",
    "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "c2db330f6083854c99d4b5bfb6e8f29f201be699",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "0098ba824b5c16427bd7a1122a5a442a25ec644d",
  ],
  [millionAs, "34aa973cd4c4daa4f61eeb2bdbad27316534016f"],
];

const testSetBase64 = [
  ["", "2jmj7l5rSw0yVb/vlWAYkK/YBwk="],
  ["abc", "qZk+NkcGgWq6PiVxeFDCbJzQ2J0="],
  ["deno", "uz2OcS2eetSvCNSjjz9S2Wg9WOs="],
  [
    "The quick brown fox jumps over the lazy dog",
    "L9ThxnotKPzthJ7hu3bnORuT6xI=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "wtszD2CDhUyZ1LW/tujynyAb5pk=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "AJi6gktcFkJ716ESKlpEKiXsZE0=",
  ],
  [millionAs, "NKqXPNTE2qT2Husr260nMWU0AW8="],
];

test("[hash/sha1] testSha1Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha1 = createHash("sha1");
    assertEquals(sha1.update(input).toString(), output);
    sha1.dispose();
  }
});

test("[hash/sha1] testSha1Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha1 = createHash("sha1");
    assertEquals(sha1.update(input).toString("base64"), output);
    sha1.dispose();
  }
});
