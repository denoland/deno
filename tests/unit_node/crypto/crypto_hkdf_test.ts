// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { hkdfSync } from "node:crypto";
import { assertEquals } from "@std/assert/mod.ts";
import { Buffer } from "node:buffer";
import nodeFixtures from "../testdata/crypto_digest_fixtures.json" with {
  type: "json",
};

Deno.test("crypto.hkdfSync - compare with node", async (t) => {
  const DATA = "Hello, world!";
  const SALT = "salt";
  const INFO = "info";
  const KEY_LEN = 64;

  for (const { digest, hkdf } of nodeFixtures) {
    await t.step({
      name: digest,
      ignore: digest.includes("blake"),
      fn() {
        let actual: string | null;
        try {
          actual = Buffer.from(hkdfSync(
            digest,
            DATA,
            SALT,
            INFO,
            KEY_LEN,
          )).toString("hex");
        } catch {
          actual = null;
        }
        assertEquals(actual, hkdf);
      },
    });
  }
});
