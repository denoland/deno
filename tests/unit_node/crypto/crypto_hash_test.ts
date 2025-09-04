// Copyright 2018-2025 the Deno authors. MIT license.
import { createHash, createHmac, getHashes, hash } from "node:crypto";
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
import { assert, assertEquals } from "@std/assert";

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
