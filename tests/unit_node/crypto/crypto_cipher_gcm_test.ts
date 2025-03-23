// Copyright 2018-2025 the Deno authors. MIT license.

import crypto from "node:crypto";
import { Buffer } from "node:buffer";
import testVectors128 from "./gcmEncryptExtIV128.json" with { type: "json" };
import testVectors256 from "./gcmEncryptExtIV256.json" with { type: "json" };
import { assertEquals, assertThrows } from "@std/assert";

const aesGcm = (bits: string, key: Uint8Array) => {
  const ALGO = bits == "128" ? `aes-128-gcm` : `aes-256-gcm`;

  // encrypt returns base64-encoded ciphertext
  const encrypt = (
    iv: Uint8Array,
    str: string,
    aad: Uint8Array,
  ): [string, Buffer] => {
    const cipher = crypto.createCipheriv(ALGO, key, iv);
    cipher.setAAD(aad);
    let enc = cipher.update(str, "base64", "base64");
    enc += cipher.final("base64");
    return [enc, cipher.getAuthTag()];
  };

  const decrypt = (
    enc: string,
    iv: Uint8Array,
    aad: Uint8Array,
    authTag: Uint8Array,
  ) => {
    const decipher = crypto.createDecipheriv(ALGO, key, iv);
    decipher.setAuthTag(authTag);
    decipher.setAAD(aad);
    let str = decipher.update(enc, "base64", "base64");
    str += decipher.final("base64");

    return str;
  };

  return {
    encrypt,
    decrypt,
  };
};

type TestVector = {
  key: Uint8Array;
  nonce: Uint8Array;
  aad: Uint8Array;
  plaintext: string;
  ciphertext: string;
  tag: Uint8Array;
};

for (
  // NIST CAVS vectors
  const [bits, vectors] of Object.entries({
    // <https://csrc.nist.gov/Projects/cryptographic-algorithm-validation-program/CAVP-TESTING-BLOCK-CIPHER-MODES>
    //
    // From: `gcmEncryptExtIV128.rsp`
    128: testVectors128,
    // <https://csrc.nist.gov/Projects/cryptographic-algorithm-validation-program/CAVP-TESTING-BLOCK-CIPHER-MODES>
    //
    // From: `gcmEncryptExtIV256.rsp`
    256: testVectors256,
  })
) {
  for (let i = 0; i < vectors.length; i++) {
    const rawTest = vectors[i];
    const test: TestVector = {
      key: new Uint8Array(rawTest.key),
      nonce: new Uint8Array(rawTest.nonce),
      aad: new Uint8Array(rawTest.aad),
      plaintext: Buffer.from(rawTest.plaintext).toString("base64"),
      ciphertext: Buffer.from(rawTest.ciphertext).toString("base64"),
      tag: new Uint8Array(rawTest.tag),
    };

    Deno.test({
      name: `aes-${bits}-gcm encrypt ${i + 1}/${vectors.length}`,
      fn() {
        const cipher = aesGcm(bits, test.key);
        const [enc, tag] = cipher.encrypt(test.nonce, test.plaintext, test.aad);
        assertEquals(enc, test.ciphertext);
        assertEquals(new Uint8Array(tag), test.tag);
      },
    });

    Deno.test({
      name: `aes-${bits}-gcm decrypt ${i + 1}/${vectors.length}`,
      fn() {
        const cipher = aesGcm(bits, test.key);
        const plaintext = cipher.decrypt(
          test.ciphertext,
          test.nonce,
          test.aad,
          test.tag,
        );
        assertEquals(plaintext, test.plaintext);
      },
    });
  }
}

Deno.test({
  name: "aes-128-gcm encrypt multiple",
  fn() {
    const key = Buffer.alloc(16);
    const nonce = Buffer.alloc(12);

    const gcm = crypto.createCipheriv("aes-128-gcm", key, nonce);

    assertEquals(gcm.update("hello", "utf8", "hex"), "6bedb6a20f");
    assertEquals(gcm.update("world", "utf8", "hex"), "c1cce09f4c");
    gcm.final();
    assertEquals(
      gcm.getAuthTag().toString("hex"),
      "bf6d20a38e0c828bea3de63b7ff1dfbd",
    );
  },
});

// Issue #27441
// https://github.com/denoland/deno/issues/27441
Deno.test({
  name: "aes-256-gcm supports IV of non standard length and auth tag check",
  fn() {
    const decipher = crypto.createDecipheriv(
      "aes-256-gcm",
      Buffer.from("eYLEiLFQnpjYksWTiKpwv2sKhw+WJb5Fo/aY2YqXswc=", "base64"),
      Buffer.from("k5oP3kb8tTbZaL3PxbFWN8ToOb8vfv2b1EuPz1LbmYU=", "base64"), // 256 bits IV
    );
    const decrypted = decipher.update(
      "s0/KBsFec29XLrGbAnLiNA==",
      "base64",
      "utf-8",
    );
    assertEquals(decrypted, "this is a secret");
    assertThrows(
      () => decipher.final(),
      TypeError,
      "Failed to authenticate data",
    );
  },
});

Deno.test({
  name: "aes gcm with invalid key length",
  fn() {
    assertThrows(
      () => {
        crypto.createCipheriv(
          "aes-128-gcm",
          Buffer.alloc(15),
          Buffer.alloc(12),
        );
      },
      Error,
      "Invalid key length",
    );

    assertThrows(
      () => {
        crypto.createCipheriv(
          "aes-256-gcm",
          Buffer.alloc(31),
          Buffer.alloc(12),
        );
      },
      Error,
      "Invalid key length",
    );
  },
});
