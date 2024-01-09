// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  createHash,
  createHmac,
  getHashes,
  randomFillSync,
  randomUUID,
} from "node:crypto";
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
import { assert, assertEquals } from "../../../../test_util/std/assert/mod.ts";

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

Deno.test("[node/crypto.getRandomUUID] works the same way as Web Crypto API", () => {
  assertEquals(randomUUID().length, crypto.randomUUID().length);
  assertEquals(typeof randomUUID(), typeof crypto.randomUUID());
});

Deno.test("[node/crypto.randomFillSync] supported arguments", () => {
  const buf = new Uint8Array(10);

  assert(randomFillSync(buf));
  assert(randomFillSync(buf, 0));
  // @ts-ignore: arraybuffer arguments are valid.
  assert(randomFillSync(buf.buffer));
  assert(randomFillSync(new DataView(buf.buffer)));
});
