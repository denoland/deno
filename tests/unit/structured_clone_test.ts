// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows } from "./test_util.ts";

// Basic tests for the structured clone algorithm. Mainly tests TypeScript
// typings. Actual functionality is tested in WPT.

Deno.test("self.structuredClone", async () => {
  const arrayOriginal = ["hello world"];
  const channelOriginal = new MessageChannel();
  const [arrayCloned, portTransferred] = self
    .structuredClone(
      [arrayOriginal, channelOriginal.port2] as [string[], MessagePort],
      {
        transfer: [channelOriginal.port2],
      },
    );
  assert(arrayOriginal !== arrayCloned); // not the same identity
  assertEquals(arrayCloned, arrayOriginal); // but same value
  channelOriginal.port1.postMessage("1");
  await new Promise((resolve) => portTransferred.onmessage = () => resolve(1));
  channelOriginal.port1.close();
  portTransferred.close();
});

Deno.test("correct DataCloneError message", () => {
  assertThrows(
    () => {
      const sab = new SharedArrayBuffer(1024);
      structuredClone(sab, {
        // @ts-expect-error cannot assign SharedArrayBuffer because it's not tranferable
        transfer: [sab],
      });
    },
    DOMException,
    "Value not transferable",
  );

  const ab = new ArrayBuffer(1);
  // detach ArrayBuffer
  structuredClone(ab, { transfer: [ab] });
  assertThrows(
    () => {
      structuredClone(ab, { transfer: [ab] });
    },
    DOMException,
    "ArrayBuffer at index 0 is already detached",
  );

  const ab2 = new ArrayBuffer(0);
  assertThrows(
    () => {
      structuredClone([ab2, ab], { transfer: [ab2, ab] });
    },
    DOMException,
    "ArrayBuffer at index 1 is already detached",
  );

  // ab2 should not be detached after above failure
  structuredClone(ab2, { transfer: [ab2] });
});

Deno.test("structuredClone CryptoKey", async () => {
  // AES key
  const aesKey = await crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    true,
    ["encrypt", "decrypt"],
  );
  const aesClone = structuredClone(aesKey);
  assert(aesKey !== aesClone);
  assertEquals(aesClone.type, aesKey.type);
  assertEquals(aesClone.extractable, aesKey.extractable);
  assertEquals(aesClone.algorithm, aesKey.algorithm);
  assertEquals([...aesClone.usages], [...aesKey.usages]);

  // Verify the cloned key actually works
  const data = new TextEncoder().encode("hello");
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encrypted = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    aesClone,
    data,
  );
  const decrypted = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    aesKey,
    encrypted,
  );
  assertEquals(new Uint8Array(decrypted), data);

  // Non-extractable key can be cloned
  const nonExtractable = await crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
  const nonExtractableClone = structuredClone(nonExtractable);
  assertEquals(nonExtractableClone.extractable, false);
  assertEquals(nonExtractableClone.algorithm, nonExtractable.algorithm);

  // HMAC key
  const hmacKey = await crypto.subtle.generateKey(
    { name: "HMAC", hash: "SHA-256" },
    true,
    ["sign", "verify"],
  );
  const hmacClone = structuredClone(hmacKey);
  assertEquals(hmacClone.type, hmacKey.type);
  assertEquals(hmacClone.algorithm, hmacKey.algorithm);
  assertEquals([...hmacClone.usages], [...hmacKey.usages]);

  // EC key pair
  const ecKeyPair = await crypto.subtle.generateKey(
    { name: "ECDSA", namedCurve: "P-256" },
    true,
    ["sign", "verify"],
  ) as CryptoKeyPair;
  const ecPrivateClone = structuredClone(ecKeyPair.privateKey);
  const ecPublicClone = structuredClone(ecKeyPair.publicKey);
  assertEquals(ecPrivateClone.type, "private");
  assertEquals(ecPublicClone.type, "public");

  // Ed25519 key pair
  const edKeyPair = await crypto.subtle.generateKey(
    "Ed25519",
    true,
    ["sign", "verify"],
  ) as CryptoKeyPair;
  const edClone = structuredClone(edKeyPair.privateKey);
  assertEquals(edClone.type, "private");
  assertEquals(edClone.algorithm.name, "Ed25519");
});
