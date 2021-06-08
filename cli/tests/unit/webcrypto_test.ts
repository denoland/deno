import { assert, assertEquals, unitTest } from "./test_util.ts";

// Close all crypto key resources to avoid resource leaks.
function closeResource() {
  const resources = Deno.resources();

  for (const i of Object.keys(resources)) {
    const rid = Number(i);
    if (
      ["RSAPublicKey", "RSAPrivateKey", "EcdsaKeyPair", "HmacKey"].includes(
        resources[rid],
      )
    ) {
      Deno.close(rid);
    }
  }
}

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
  closeResource();
});

unitTest(async function testGenerateHMACKey() {
  const key = await window.crypto.subtle.generateKey(
    {
      name: "HMAC",
      hash: "SHA-512",
    },
    true,
    ["sign", "verify"],
  );

  assert(key);
  assertEquals(key.extractable, true);
  assert(key.usages.includes("sign"));
  closeResource();
});
