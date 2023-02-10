// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Calvin Metcalf. All rights reserved. MIT license.

import * as crypto from "../mod.js";
import fs from "../../../../fs.ts";
import path from "../../../../path.ts";
import { Buffer } from "../../../../buffer.ts";
import { assertEquals, assertThrows } from "../../../../../testing/asserts.ts";

// Test RSA encryption/decryption
Deno.test("node tests", function () {
  const keyPem = fs.readFileSync(
    path.fromFileUrl(new URL("test_key.pem", import.meta.url)),
    "ascii",
  );
  const rsaPubPem = fs.readFileSync(
    path.fromFileUrl(new URL("test_rsa_pubkey.pem", import.meta.url)),
    "ascii",
  );
  const rsaKeyPem = fs.readFileSync(
    path.fromFileUrl(new URL("test_rsa_privkey.pem", import.meta.url)),
    "ascii",
  );
  const rsaKeyPemEncrypted = fs.readFileSync(
    path.fromFileUrl(
      new URL("test_rsa_privkey_encrypted.pem", import.meta.url),
    ),
    "ascii",
  );
  const input = "I AM THE WALRUS";
  const bufferToEncrypt = Buffer.from(input);

  let encryptedBuffer = crypto.publicEncrypt(rsaPubPem, bufferToEncrypt);

  let decryptedBuffer = crypto.privateDecrypt(rsaKeyPem, encryptedBuffer);
  assertEquals(input, decryptedBuffer.toString());

  const decryptedBufferWithPassword = crypto.privateDecrypt({
    key: rsaKeyPemEncrypted,
    passphrase: "password",
  }, encryptedBuffer);
  assertEquals(input, decryptedBufferWithPassword.toString());

  encryptedBuffer = crypto.publicEncrypt(keyPem, bufferToEncrypt);

  decryptedBuffer = crypto.privateDecrypt(keyPem, encryptedBuffer);
  assertEquals(input, decryptedBuffer.toString());

  encryptedBuffer = crypto.privateEncrypt(keyPem, bufferToEncrypt);

  decryptedBuffer = crypto.publicDecrypt(keyPem, encryptedBuffer);
  assertEquals(input, decryptedBuffer.toString());

  assertThrows(function () {
    crypto.privateDecrypt({
      key: rsaKeyPemEncrypted,
      passphrase: "wrong",
    }, encryptedBuffer);
  });
});
