// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "9c1185a5c5e9fc54612808977ee8f548b2258d31"],
  ["abc", "8eb208f7e05d987a9b044a8e98c6b087f15a0bfc"],
  ["deno", "dc3c354a2004fc9bf46c64729e9b556eb414b812"],
  [
    "The quick brown fox jumps over the lazy dog",
    "37f332f68db77bd9d7edd4969571ad671cf9dd3b",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "e72334b46c83cc70bef979e15453706c95b888be",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "9dfb7d374ad924f3f88de96291c33e9abed53e32",
  ],
  [millionAs, "52783243c1697bdbe16d37f97f68f08325dc1528"],
];

const testSetBase64 = [
  ["", "nBGFpcXp/FRhKAiXfuj1SLIljTE="],
  ["abc", "jrII9+BdmHqbBEqOmMawh/FaC/w="],
  ["deno", "3Dw1SiAE/Jv0bGRynptVbrQUuBI="],
  [
    "The quick brown fox jumps over the lazy dog",
    "N/My9o23e9nX7dSWlXGtZxz53Ts=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "5yM0tGyDzHC++XnhVFNwbJW4iL4=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "nft9N0rZJPP4jelikcM+mr7VPjI=",
  ],
  [millionAs, "UngyQ8Fpe9vhbTf5f2jwgyXcFSg="],
];

test("[hash/ripemd160] testRipemd160Hex", () => {
  for (const [input, output] of testSetHex) {
    const ripemd160 = createHash("ripemd160");
    assertEquals(ripemd160.update(input).toString(), output);
    ripemd160.dispose();
  }
});

test("[hash/ripemd160] testRipemd160Base64", () => {
  for (const [input, output] of testSetBase64) {
    const ripemd160 = createHash("ripemd160");
    assertEquals(ripemd160.update(input).toString("base64"), output);
    ripemd160.dispose();
  }
});
