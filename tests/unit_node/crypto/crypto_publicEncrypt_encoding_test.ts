// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for the `encoding` option on `crypto.publicEncrypt` /
// `privateDecrypt` / `privateEncrypt` / `publicDecrypt`. When the key is
// passed as `{ key, encoding }`, Node applies that encoding to the buffer
// argument too. Prior to this fix, Deno applied the encoding only to the
// key string, so a hex-encoded plaintext was treated as UTF-8 bytes.
//
// Mirrors the shape exercised by Node's `parallel/test-crypto-rsa-dsa.js`.

import {
  generateKeyPairSync,
  privateDecrypt,
  privateEncrypt,
  publicDecrypt,
  publicEncrypt,
} from "node:crypto";
import { Buffer } from "node:buffer";
import { assertEquals } from "../../unit/test_util.ts";

function makeKeys() {
  const { publicKey: publicPem, privateKey: privatePem } = generateKeyPairSync(
    "rsa",
    {
      modulusLength: 2048,
      publicKeyEncoding: { type: "spki", format: "pem" },
      privateKeyEncoding: { type: "pkcs8", format: "pem" },
    },
  );
  return {
    publicPem,
    privatePem,
    publicPemBytes: Buffer.from(publicPem),
    privatePemBytes: Buffer.from(privatePem),
  };
}

const plaintext = "I AM THE WALRUS";

Deno.test("[node/crypto.publicEncrypt] hex-encoded buffer arg is decoded", () => {
  const { publicPemBytes, privatePem } = makeKeys();
  const hex = Buffer.from(plaintext).toString("hex");
  // deno-lint-ignore no-explicit-any
  const ct = publicEncrypt(
    { key: publicPemBytes, encoding: "hex" } as any,
    hex,
  );
  const pt = privateDecrypt(privatePem, ct);
  assertEquals(pt.toString(), plaintext);
});

Deno.test("[node/crypto.publicEncrypt] base64-encoded buffer arg is decoded", () => {
  const { publicPemBytes, privatePem } = makeKeys();
  const b64 = Buffer.from(plaintext).toString("base64");
  // deno-lint-ignore no-explicit-any
  const ct = publicEncrypt(
    { key: publicPemBytes, encoding: "base64" } as any,
    b64,
  );
  const pt = privateDecrypt(privatePem, ct);
  assertEquals(pt.toString(), plaintext);
});

Deno.test("[node/crypto.publicEncrypt] hex applies to both key string and buffer", () => {
  // Matches the upstream `parallel/test-crypto-rsa-dsa.js` snippet.
  const { publicPemBytes, privatePem } = makeKeys();
  const keyHex = publicPemBytes.toString("hex");
  const dataHex = Buffer.from(plaintext).toString("hex");
  // deno-lint-ignore no-explicit-any
  const ct = publicEncrypt({ key: keyHex, encoding: "hex" } as any, dataHex);
  const pt = privateDecrypt(privatePem, ct);
  assertEquals(pt.toString(), plaintext);
});

Deno.test("[node/crypto.publicEncrypt] no encoding -> buffer string is utf8", () => {
  const { publicPem, privatePem } = makeKeys();
  const ct = publicEncrypt(publicPem, plaintext);
  const pt = privateDecrypt(privatePem, ct);
  assertEquals(pt.toString(), plaintext);
});

Deno.test("[node/crypto.privateEncrypt] hex round-trips through publicDecrypt", () => {
  const { publicPemBytes, privatePemBytes } = makeKeys();
  const hex = Buffer.from(plaintext).toString("hex");
  const ct = privateEncrypt(
    // deno-lint-ignore no-explicit-any
    { key: privatePemBytes, encoding: "hex" } as any,
    hex,
  );
  const pt = publicDecrypt(
    // deno-lint-ignore no-explicit-any
    { key: publicPemBytes, encoding: "hex" } as any,
    ct,
  );
  assertEquals(pt.toString(), plaintext);
});
