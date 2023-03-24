// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { createSecretKey, randomBytes } from "node:crypto";
import { Buffer } from "node:buffer";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import { createHmac } from "node:crypto";

Deno.test({
  name: "create secret key",
  fn() {
    const key = createSecretKey(Buffer.alloc(0));
    assertEquals(key.type, "secret");
    assertEquals(key.asymmetricKeyType, undefined);
    assertEquals(key.symmetricKeySize, 0);
  },
});

Deno.test({
  name: "export secret key",
  fn() {
    const material = Buffer.from(randomBytes(32));
    const key = createSecretKey(material);
    assertEquals(Buffer.from(key.export()), material);
  },
});

Deno.test({
  name: "export jwk secret key",
  fn() {
    const material = Buffer.from("secret");
    const key = createSecretKey(material);
    assertEquals(key.export({ format: "jwk" }), {
      kty: "oct",
      k: "c2VjcmV0",
    });
  },
});

Deno.test({
  name: "createHmac with secret key",
  fn() {
    const key = createSecretKey(Buffer.from("secret"));
    assertEquals(
      createHmac("sha256", key).update("hello").digest().toString("hex"),
      "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b",
    );
  },
});
