// Copyright 2018-2026 the Deno authors. MIT license.
import { createHash, createHmac, getHashes, hash } from "node:crypto";
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
import { assert, assertEquals, assertThrows } from "@std/assert";

// https://github.com/denoland/deno/issues/18140
Deno.test({
  name: "[node/crypto] createHmac digest",
  fn() {
    assertEquals(
      createHmac("sha256", "secret").update("hello").digest("hex"),
      "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac sha512-224",
  fn() {
    assertEquals(
      createHmac("sha512-224", "secret").update("hello").digest("hex"),
      "27ade3215d20a0e939a1ff98f91052148e85f2ece87d926d6a2c1aad",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac sha512-256",
  fn() {
    assertEquals(
      createHmac("sha512-256", "secret").update("hello").digest("hex"),
      "e1a285d0317f7cce89acb5642fb6e82fc16d14ab588b0a5abcc7c20ea748594e",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHash digest",
  fn() {
    assertEquals(
      createHash("sha256").update("hello").digest("hex"),
      "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
    );
  },
});

Deno.test("[node/crypto.Hash] basic usage - buffer output", () => {
  const d = createHash("sha1").update("abc").update("def").digest();
  assertEquals(
    d,
    Buffer.from([
      0x1f,
      0x8a,
      0xc1,
      0xf,
      0x23,
      0xc5,
      0xb5,
      0xbc,
      0x11,
      0x67,
      0xbd,
      0xa8,
      0x4b,
      0x83,
      0x3e,
      0x5c,
      0x5,
      0x7a,
      0x77,
      0xd2,
    ]),
  );
});

Deno.test("[node/crypto.Hash] basic usage - hex output", () => {
  const d = createHash("sha1").update("abc").update("def").digest("hex");
  assertEquals(d, "1f8ac10f23c5b5bc1167bda84b833e5c057a77d2");
});

Deno.test("[node/crypto.Hash] basic usage - base64 output", () => {
  const d = createHash("sha1").update("abc").update("def").digest("base64");
  assertEquals(d, "H4rBDyPFtbwRZ72oS4M+XAV6d9I=");
});

Deno.test("[node/crypto.Hash] basic usage - base64url output", () => {
  const d = createHash("sha1").update("abc").update("def").digest("base64url");
  assertEquals(d, "H4rBDyPFtbwRZ72oS4M-XAV6d9I");
});

Deno.test("[node/crypto.Hash] streaming usage", async () => {
  const source = Readable.from(["abc", "def"]);
  const hash = createHash("sha1");
  const dest = source.pipe(hash);
  const result = await new Promise((resolve, _) => {
    let buffer = Buffer.from([]);
    dest.on("data", (data) => {
      buffer = Buffer.concat([buffer, data]);
    });
    dest.on("end", () => {
      resolve(buffer);
    });
  });
  assertEquals(
    result,
    Buffer.from([
      0x1f,
      0x8a,
      0xc1,
      0xf,
      0x23,
      0xc5,
      0xb5,
      0xbc,
      0x11,
      0x67,
      0xbd,
      0xa8,
      0x4b,
      0x83,
      0x3e,
      0x5c,
      0x5,
      0x7a,
      0x77,
      0xd2,
    ]),
  );
});

Deno.test("[node/crypto.getHashes]", () => {
  for (const algorithm of getHashes()) {
    const d = createHash(algorithm).update("abc").digest();
    assert(d instanceof Buffer);
    assert(d.length > 0);
  }
});

Deno.test("[node/crypto.hash] supports buffer args", () => {
  const buffer = Buffer.from("abc");
  const d = createHash("sha1").update(buffer).digest("hex");
  assertEquals(d, "a9993e364706816aba3e25717850c26c9cd0d89d");
});

Deno.test("[node/crypto.hash] does not leak", () => {
  const hasher = createHash("sha1");
  hasher.update("abc");
});

Deno.test("[node/crypto.hash] oneshot hash API", () => {
  const d = hash("sha1", "Node.js");
  assertEquals(d, "10b3493287f831e81a438811a1ffba01f8cec4b7");
});

Deno.test("[node/crypto.hash] shake-128 alias", () => {
  const d = hash("shake-128", "Node.js", "base64url");
  assertEquals(d, "Nkx9-EgHpFkeXY5OPsL0rg");
});

Deno.test("[node/crypto.hash] shake-256 alias", () => {
  const d = hash("shake-256", "Node.js", "base64url");
  assertEquals(d, "JdelDxiwp92tkk9jYjEFPMlHD0gC8bMbYtHRCIM6TTQ");
});

Deno.test("[node/crypto.createHmac] should not print deprecation warning", async () => {
  const script =
    'import crypto from "node:crypto"; crypto.createHmac("SHA256", "foo")';

  const child = new Deno.Command(Deno.execPath(), {
    args: ["eval", script],
    stdout: "piped",
    stderr: "piped",
  }).spawn();

  const { code, stderr } = await child.output();
  assertEquals(code, 0);

  const decodedStderr = new TextDecoder().decode(stderr).trim();
  assertEquals(decodedStderr, "");
});

Deno.test({
  name: "[node/crypto] createHmac sha3-224",
  fn() {
    assertEquals(
      createHmac("sha3-224", "secret").update("hello").digest("hex"),
      "d078791e9bf080c2139f883ac65033d4b5b75bbdb4088c494d0b6a14",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac sha3-256",
  fn() {
    assertEquals(
      createHmac("sha3-256", "secret").update("hello").digest("hex"),
      "850ae61707b3e60d4e45548c4facfda415d301712641fd11535cf395d9e2d7fe",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac sha3-384",
  fn() {
    assertEquals(
      createHmac("sha3-384", "secret").update("hello").digest("hex"),
      "e24e0dc664132644a6740071af5a05622edffea8afacf0a4060111961bc9148f23c001b6f7d7e79a44b9896b1f00cd85",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac sha3-512",
  fn() {
    assertEquals(
      createHmac("sha3-512", "secret").update("hello").digest("hex"),
      "bc07c2dfc0295b420662bda474eb8db11b0389822e13da56cf9991f467f2f6c713c481aa8663900ecaee310bf2f226eaa5c2d1345dfebee990658bd529a9c504",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac blake2b512",
  fn() {
    assertEquals(
      createHmac("blake2b512", "secret").update("hello").digest("hex"),
      "59d8e60d8f7f54753ab7b823b11f20879c4db732e5b56a0da5559d10b2c2b7ac37d47474b668725b661178359ad71c189597108dd2d94ca051697fbc24b6d7ad",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac blake2s256",
  fn() {
    assertEquals(
      createHmac("blake2s256", "secret").update("hello").digest("hex"),
      "56f9d5d171c31a9481d1949743ddd370209f7c666ba8bb6872067ad70398d9ce",
    );
  },
});

Deno.test({
  name: "[node/crypto] createHmac unknown algorithm throws",
  fn() {
    assertThrows(
      () => createHmac("unknown-algorithm", "secret"),
      TypeError,
      "Invalid digest: unknown-algorithm",
    );
  },
});
