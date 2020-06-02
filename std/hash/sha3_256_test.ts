// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"],
  ["abc", "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"],
  ["deno", "74a6286af90f8775d74080f864cf80b11eecf6f14d325c5ef8c9f7ccc8055517"],
  [
    "The quick brown fox jumps over the lazy dog",
    "69070dda01975c8c120c3aada1b282394e7f032fa9cf32f4cb2259a0897dfc04",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "f6fe8de5c8f5014786f07e9f7b08130f920dd55e587d47021686b26cf2323deb",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "043d104b5480439c7acff8831ee195183928d9b7f8fcb0c655a086a87923ffee",
  ],
  [
    millionAs,
    "5c8875ae474a3634ba4fd55ec85bffd661f32aca75c6d699d0cdcb6c115891c1",
  ],
];

const testSetBase64 = [
  ["", "p//G+L8e12ZRwUdWoGHWYvWA/03kO0n6gtgKS4D4Q0o="],
  ["abc", "Ophdp0/iJbIEXBcta9OQvYVfCG4+nVJbRr/iRRFDFTI="],
  ["deno", "dKYoavkPh3XXQID4ZM+AsR7s9vFNMlxe+Mn3zMgFVRc="],
  [
    "The quick brown fox jumps over the lazy dog",
    "aQcN2gGXXIwSDDqtobKCOU5/Ay+pzzL0yyJZoIl9/AQ=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "9v6N5cj1AUeG8H6fewgTD5IN1V5YfUcCFoaybPIyPes=",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "BD0QS1SAQ5x6z/iDHuGVGDko2bf4/LDGVaCGqHkj/+4=",
  ],
  [millionAs, "XIh1rkdKNjS6T9VeyFv/1mHzKsp1xtaZ0M3LbBFYkcE="],
];

test("[hash/sha3-256] testSha3-256Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha3 = createHash("sha3-256");
    assertEquals(sha3.update(input).toString(), output);
    sha3.dispose();
  }
});

test("[hash/sha3-256] testSha3-256Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha3 = createHash("sha3-256");
    assertEquals(sha3.update(input).toString("base64"), output);
    sha3.dispose();
  }
});
