// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import crypto from "node:crypto";
import { Buffer } from "node:buffer";
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
