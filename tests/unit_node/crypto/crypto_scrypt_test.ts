// Copyright 2018-2026 the Deno authors. MIT license.
import { scrypt, scryptSync } from "node:crypto";
import { Buffer } from "node:buffer";
import { assertEquals, assertThrows } from "@std/assert";

Deno.test("scrypt works correctly", async () => {
  const { promise, resolve } = Promise.withResolvers<boolean>();

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
    resolve(true);
  });

  await promise;
});

Deno.test("scrypt works with options", async () => {
  const { promise, resolve } = Promise.withResolvers<boolean>();

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
      resolve(true);
    },
  );

  await promise;
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

// Regression test for https://github.com/denoland/deno/issues/27716
Deno.test("scrypt with 128-byte keylen matches Node", async () => {
  const expected = "745731af4484f323968969eda289aeee005b5903ac561e64a5aca121" +
    "797bf7734ef9fd58422e2e22183bcacba9ec87ba0c83b7a2e788f03ce0da06463433cda" +
    "64176095fbbad7dc98c33fd75955b4b29c94f6e97617bd68d8ff17cf1ed5ad12f3fc6c8" +
    "eb5b844f2d003ebaf5eaed19e8f665928472a6941f7efc6ebcdd6fd13a";

  const { promise, resolve, reject } = Promise.withResolvers<void>();
  scrypt("password", "salt", 128, (err, key) => {
    if (err) {
      reject(err);
      return;
    }
    assertEquals(key?.toString("hex"), expected);
    resolve();
  });
  await promise;

  assertEquals(scryptSync("password", "salt", 128).toString("hex"), expected);
});

Deno.test("scrypt handles concurrent 128-byte keylen calls", async () => {
  const run = () =>
    new Promise<string>((resolve, reject) => {
      scrypt("password", "salt", 128, (err, key) => {
        if (err) reject(err);
        else resolve(key!.toString("hex"));
      });
    });

  const [a, b] = await Promise.all([run(), run()]);
  assertEquals(a, b);
});

Deno.test("scryptSync throws Node-compatible error for invalid params", () => {
  const error = assertThrows(() => {
    scryptSync("pass", "salt", 1, { N: 1, p: 1, r: 1 });
  }) as RangeError & { code?: string };
  assertEquals(error instanceof RangeError, true);
  assertEquals(error.code, "ERR_CRYPTO_INVALID_SCRYPT_PARAMS");
  assertEquals(error.message, "Invalid scrypt params");
});

Deno.test("scrypt accepts large safe maxmem values", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();

  scrypt("", "", 4, { maxmem: 2 ** 52 }, (err, key) => {
    if (err) {
      reject(err);
      return;
    }
    assertEquals(key?.toString("hex"), "d72c87d0");
    resolve();
  });

  await promise;
});
