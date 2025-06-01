// Copyright 2018-2025 the Deno authors. MIT license.
import crypto, { KeyFormat } from "node:crypto";
import path from "node:path";
import { Buffer } from "node:buffer";
import { assert } from "@std/assert/mod.ts";
import asymmetric from "./testdata/asymmetric.json" with { type: "json" };

Deno.test("crypto.createPrivateKey", async (t) => {
  for (const key of asymmetric) {
    await testCreatePrivateKey(t, key.name, "pem", "pkcs8");
    await testCreatePrivateKey(t, key.name, "der", "pkcs8");
  }
});

function testCreatePrivateKey(
  t: Deno.TestContext,
  name: string,
  format: KeyFormat,
  type: "pkcs8" | "pkcs1" | "sec1",
) {
  if (name.includes("dh")) return;
  return t.step(`crypto.createPrivateKey ${name} ${format} ${type}`, () => {
    const file = path.join(
      "./tests/unit_node/crypto/testdata/asymmetric",
      `${name}.${type}.${format}`,
    );
    const key = Buffer.from(Deno.readFileSync(file));
    const privateKey = crypto.createPrivateKey({ key, format, type });
    assert(privateKey);
  });
}
