// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
  ["abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"],
  ["deno", "e872e7bd2ae6abcf13a4c834029a342c882c1162ebf77b6720968b2000312ffb"],
  [
    "The quick brown fox jumps over the lazy dog",
    "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
  ],
  [
    millionAs,
    "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
  ],
];

const testSetBase64 = [
  ["", "47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU="],
  ["abc", "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="],
  ["deno", "6HLnvSrmq88TpMg0Apo0LIgsEWLr93tnIJaLIAAxL/s="],
  [
    "The quick brown fox jumps over the lazy dog",
    "16j7swfXgJRpypq8sAguT41WUeRtPNt2LQLQvzfJ5ZI=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "s1Q5pKxvCUi21vnjxq8PX1kM4g8b3nCQ73lwaG7Gc4o=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "/+BU/nrgy23GXDr5th1SCfQ5hR20PQulmXM33xVGaOs=",
  ],
  [millionAs, "zcduXJkU+5KBocfihNc+Z/GAmkiklyAOBG05zMcRLNA="],
];

test("[hash/sha256] testSha256Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha256 = createHash("sha256");
    assertEquals(sha256.update(input).toString(), output);
    sha256.dispose();
  }
});

test("[hash/sha256] testSha256Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha256 = createHash("sha256");
    assertEquals(sha256.update(input).toString("base64"), output);
    sha256.dispose();
  }
});
