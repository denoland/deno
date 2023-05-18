// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import crypto from "node:crypto";
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
import { buffer, text } from "node:stream/consumers";
import {
  assertEquals,
  assertThrows,
} from "../../../test_util/std/testing/asserts.ts";

const rsaPrivateKey = Deno.readTextFileSync(
  new URL("./testdata/rsa_private.pem", import.meta.url),
);
const rsaPublicKey = Deno.readTextFileSync(
  new URL("./testdata/rsa_public.pem", import.meta.url),
);

const input = new TextEncoder().encode("hello world");

function zeros(length: number): Uint8Array {
  return new Uint8Array(length);
}

Deno.test({
  name: "rsa public encrypt and private decrypt",
  fn() {
    const encrypted = crypto.publicEncrypt(Buffer.from(rsaPublicKey), input);
    const decrypted = crypto.privateDecrypt(
      Buffer.from(rsaPrivateKey),
      Buffer.from(encrypted),
    );
    assertEquals(decrypted, input);
  },
});

Deno.test({
  name: "rsa private encrypt and private decrypt",
  fn() {
    const encrypted = crypto.privateEncrypt(rsaPrivateKey, input);
    const decrypted = crypto.privateDecrypt(
      rsaPrivateKey,
      Buffer.from(encrypted),
    );
    assertEquals(decrypted, input);
  },
});

Deno.test({
  name: "rsa public decrypt fail",
  fn() {
    const encrypted = crypto.publicEncrypt(rsaPublicKey, input);
    assertThrows(() =>
      crypto.publicDecrypt(rsaPublicKey, Buffer.from(encrypted))
    );
  },
});

Deno.test({
  name: "createCipheriv - multiple chunk inputs",
  fn() {
    const cipher = crypto.createCipheriv(
      "aes-128-cbc",
      new Uint8Array(16),
      new Uint8Array(16),
    );
    assertEquals(
      cipher.update(new Uint8Array(16), undefined, "hex"),
      "66e94bd4ef8a2c3b884cfa59ca342b2e",
    );
    assertEquals(
      cipher.update(new Uint8Array(19), undefined, "hex"),
      "f795bd4a52e29ed713d313fa20e98dbc",
    );
    assertEquals(
      cipher.update(new Uint8Array(55), undefined, "hex"),
      "a10cf66d0fddf3405370b4bf8df5bfb347c78395e0d8ae2194da0a90abc9888a94ee48f6c78fcd518a941c3896102cb1",
    );
    assertEquals(cipher.final("hex"), "e11901dde4a2f99fe4efc707e48c6aed");
  },
});

Deno.test({
  name: "createCipheriv - algorithms",
  fn() {
    const table = [
      [
        ["aes-128-cbc", 16, 16],
        "66e94bd4ef8a2c3b884cfa59ca342b2ef795bd4a52e29ed713d313fa20e98dbca10cf66d0fddf3405370b4bf8df5bfb3",
        "d5f65ecda64511e9d3d12206411ffd72",
      ],
      [
        ["aes-128-ecb", 16, 0],
        "66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e",
        "baf823258ca2e6994f638daa3515e986",
      ],
    ] as const;
    for (
      const [[alg, keyLen, ivLen], expectedUpdate, expectedFinal] of table
    ) {
      const cipher = crypto.createCipheriv(alg, zeros(keyLen), zeros(ivLen));
      assertEquals(cipher.update(zeros(50), undefined, "hex"), expectedUpdate);
      assertEquals(cipher.final("hex"), expectedFinal);
    }
  },
});

Deno.test({
  name: "createCipheriv - input encoding",
  fn() {
    const cipher = crypto.createCipheriv(
      "aes-128-cbc",
      new Uint8Array(16),
      new Uint8Array(16),
    );
    assertEquals(
      cipher.update("hello, world! hello, world!", "utf-8", "hex"),
      "ca7df4d74f51b77a7440ead38343ab0f",
    );
    assertEquals(cipher.final("hex"), "d0da733dec1fa61125c80a6f97e6166e");
  },
});

Deno.test({
  name: "createCipheriv - transform stream",
  async fn() {
    const result = await buffer(
      Readable.from("foo".repeat(15)).pipe(crypto.createCipheriv(
        "aes-128-cbc",
        new Uint8Array(16),
        new Uint8Array(16),
      )),
    );
    // deno-fmt-ignore
    assertEquals([...result], [
      129,  19, 202, 142, 137,  51,  23,  53, 198,  33,
      214, 125,  17,   5, 128,  57, 162, 217, 220,  53,
      172,  51,  85, 113,  71, 250,  44, 156,  80,   4,
      158,  92, 185, 173,  67,  47, 255,  71,  78, 187,
       80, 206,  42,   5,  34, 104,   1,  54
    ]);
  },
});

Deno.test({
  name: "createDecipheriv - algorithms",
  fn() {
    const table = [
      [
        ["aes-128-cbc", 16, 16],
        "66e94bd4ef8a2c3b884cfa59ca342b2ef795bd4a52e29ed713d313fa20e98dbca10cf66d0fddf3405370b4bf8df5bfb347c78395e0d8ae2194da0a90abc9888a94ee48f6c78fcd518a941c3896102cb1e11901dde4a2f99fe4efc707e48c6aed",
      ],
      [
        ["aes-128-ecb", 16, 0],
        "66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2e66e94bd4ef8a2c3b884cfa59ca342b2ec29a917cbaf72fa9bc32129bb0d17663",
      ],
    ] as const;
    for (
      const [[alg, keyLen, ivLen], input] of table
    ) {
      const cipher = crypto.createDecipheriv(alg, zeros(keyLen), zeros(ivLen));
      assertEquals(cipher.update(input, "hex"), Buffer.alloc(80));
      assertEquals(cipher.final(), Buffer.alloc(10));
    }
  },
});

Deno.test({
  name: "createDecipheriv - transform stream",
  async fn() {
    const stream = Readable.from([
      // deno-fmt-ignore
      new Uint8Array([
        129,  19, 202, 142, 137,  51,  23,  53, 198,  33,
        214, 125,  17,   5, 128,  57, 162, 217, 220,  53,
        172,  51,  85, 113,  71, 250,  44, 156,  80,   4,
        158,  92, 185, 173,  67,  47, 255,  71,  78, 187,
         80, 206,  42,   5,  34, 104,   1,  54
      ]),
    ]).pipe(crypto.createDecipheriv(
      "aes-128-cbc",
      new Uint8Array(16),
      new Uint8Array(16),
    ));
    assertEquals(await text(stream), "foo".repeat(15));
  },
});
