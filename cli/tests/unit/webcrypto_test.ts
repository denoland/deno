import { assert, assertEquals, unitTest } from "./test_util.ts";

unitTest(async function testGenerateRSAKey() {
  const subtle = window.crypto.subtle;
  assert(subtle);

  const keyPair = await subtle.generateKey(
    {
      name: "RSA-PSS",
      modulusLength: 2048,
      publicExponent: new Uint8Array([1, 0, 1]),
      hash: "SHA-256",
    },
    true,
    ["sign", "verify"],
  );

  assert(keyPair.privateKey);
  assert(keyPair.publicKey);
  assertEquals(keyPair.privateKey.extractable, true);
  assert(keyPair.privateKey.usages.includes("sign"));
});

unitTest(async function testGenerateHMACKey() {
  const key = await window.crypto.subtle.generateKey(
    {
      name: "HMAC",
      hash: { name: "SHA-512" },
    },
    true,
    ["sign", "verify"],
  );

  assert(key);
  assertEquals(key.extractable, true);
  assert(key.usages.includes("sign"));
});

unitTest(async function testSignECDSA() {
  const key = await window.crypto.subtle.generateKey(
    {
      name: "ECDSA",
      namedCurve: "P-384",
    },
    true,
    ["sign", "verify"],
  );

  const encoder = new TextEncoder();
  const encoded = encoder.encode("Hello, World!");
  const signature = await window.crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-384" },
    key.privateKey,
    encoded,
  );

  assert(signature);
});
