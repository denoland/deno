// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { Md5 } from "./md5.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  ["", "d41d8cd98f00b204e9800998ecf8427e"],
  ["abc", "900150983cd24fb0d6963f7d28e17f72"],
  ["deno", "c8772b401bc911da102a5291cc4ec83b"],
  [
    "The quick brown fox jumps over the lazy dog",
    "9e107d9d372bb6826bd81d3542a419d6",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "3b0c8ac703f828b04c6c197006d17218",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "014842d480b571495a4a0363793f7367",
  ],
  [millionAs, "7707d6ae4e027c70eea2a935c2296f21"],
];

const testSetBase64 = [
  ["", "1B2M2Y8AsgTpgAmY7PhCfg=="],
  ["abc", "kAFQmDzST7DWlj99KOF/cg=="],
  ["deno", "yHcrQBvJEdoQKlKRzE7IOw=="],
  ["The quick brown fox jumps over the lazy dog", "nhB9nTcrtoJr2B01QqQZ1g=="],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "OwyKxwP4KLBMbBlwBtFyGA==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "AUhC1IC1cUlaSgNjeT9zZw==",
  ],
  [millionAs, "dwfWrk4CfHDuoqk1wilvIQ=="],
];

Deno.test("[hash/md5] testMd5Hex", () => {
  for (const [input, output] of testSetHex) {
    const md5 = new Md5();
    assertEquals(md5.update(input).toString(), output);
  }
});

Deno.test("[hash/md5] testMd5Base64", () => {
  for (const [input, output] of testSetBase64) {
    const md5 = new Md5();
    assertEquals(md5.update(input).toString("base64"), output);
  }
});
