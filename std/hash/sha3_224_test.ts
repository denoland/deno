// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "6b4e03423667dbb73b6e15454f0eb1abd4597f9a1b078e3f5b5a6bc7"],
  ["abc", "e642824c3f8cf24ad09234ee7d3c766fc9a3a5168d0c94ad73b46fdf"],
  ["deno", "4da3f5328887217780db9790d71a978e2ad19927616ba8863d79ce33"],
  [
    "The quick brown fox jumps over the lazy dog",
    "d15dadceaa4d5d7bb3b48f446421d542e08ad8887305e28d58335795",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "08d654d94751580d7730b56064734b662eff7b2d159bed9ad55c935c",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "ce06309528da04278a8072ca96610a47298cbca3a9b6a0ee7f581316",
  ],
  [millionAs, "d69335b93325192e516a912e6d19a15cb51c6ed5c15243e7a7fd653c"],
];

const testSetBase64 = [
  ["", "a04DQjZn27c7bhVFTw6xq9RZf5obB44/W1prxw=="],
  ["abc", "5kKCTD+M8krQkjTufTx2b8mjpRaNDJStc7Rv3w=="],
  ["deno", "TaP1MoiHIXeA25eQ1xqXjirRmSdha6iGPXnOMw=="],
  [
    "The quick brown fox jumps over the lazy dog",
    "0V2tzqpNXXuztI9EZCHVQuCK2IhzBeKNWDNXlQ==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "CNZU2UdRWA13MLVgZHNLZi7/ey0Vm+2a1VyTXA==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "zgYwlSjaBCeKgHLKlmEKRymMvKOptqDuf1gTFg==",
  ],
  [millionAs, "1pM1uTMlGS5RapEubRmhXLUcbtXBUkPnp/1lPA=="],
];

test("[hash/sha3-224] testSha3-224Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha3 = createHash("sha3-224");
    assertEquals(sha3.update(input).toString(), output);
    sha3.dispose();
  }
});

test("[hash/sha3-224] testSha3-224Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha3 = createHash("sha3-224");
    assertEquals(sha3.update(input).toString("base64"), output);
    sha3.dispose();
  }
});
