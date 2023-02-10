// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { scrypt, scryptSync } from "./scrypt.ts";
import { Buffer } from "../../buffer.ts";
import { assertEquals } from "../../../testing/asserts.ts";

Deno.test("scrypt works correctly", () => {
  scrypt("password", "salt", 32, (err, key) => {
    if (err) throw err;
    assertEquals(
      key,
      Buffer.from([
        116,
        87,
        49,
        175,
        68,
        132,
        243,
        35,
        150,
        137,
        105,
        237,
        162,
        137,
        174,
        238,
        0,
        91,
        89,
        3,
        172,
        86,
        30,
        100,
        165,
        172,
        161,
        33,
        121,
        123,
        247,
        115,
      ]),
    );
  });
});

Deno.test("scrypt works with options", () => {
  scrypt(
    "password",
    "salt",
    32,
    {
      N: 512,
    },
    (err, key) => {
      if (err) throw err;
      assertEquals(
        key,
        Buffer.from([
          57,
          134,
          165,
          72,
          236,
          9,
          166,
          182,
          42,
          46,
          138,
          230,
          251,
          154,
          25,
          15,
          214,
          209,
          57,
          208,
          31,
          163,
          203,
          87,
          251,
          42,
          144,
          179,
          98,
          92,
          193,
          71,
        ]),
      );
    },
  );
});

Deno.test("scryptSync works correctly", () => {
  const key = scryptSync("password", "salt", 32);
  assertEquals(
    key,
    Buffer.from([
      116,
      87,
      49,
      175,
      68,
      132,
      243,
      35,
      150,
      137,
      105,
      237,
      162,
      137,
      174,
      238,
      0,
      91,
      89,
      3,
      172,
      86,
      30,
      100,
      165,
      172,
      161,
      33,
      121,
      123,
      247,
      115,
    ]),
  );
});

Deno.test("scryptSync with options works correctly", () => {
  const key = scryptSync("password", "salt", 32, { N: 512 });
  assertEquals(
    key,
    Buffer.from([
      57,
      134,
      165,
      72,
      236,
      9,
      166,
      182,
      42,
      46,
      138,
      230,
      251,
      154,
      25,
      15,
      214,
      209,
      57,
      208,
      31,
      163,
      203,
      87,
      251,
      42,
      144,
      179,
      98,
      92,
      193,
      71,
    ]),
  );
});
