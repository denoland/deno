import { assert, assertEquals, unitTest } from "./test_util.ts";

// https://github.com/denoland/deno/issues/11664
unitTest(async function testImportArrayBufferKey() {
  const subtle = window.crypto.subtle;
  assert(subtle);

  // deno-fmt-ignore
  const key = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);

  const cryptoKey = await subtle.importKey(
    "raw",
    key.buffer,
    { name: "HMAC", hash: "SHA-1" },
    true,
    ["sign"],
  );
  assert(cryptoKey);

  // Test key usage
  await subtle.sign({ name: "HMAC" }, cryptoKey, new Uint8Array(8));
});

// TODO(@littledivy): Remove this when we enable WPT for sign_verify
unitTest(async function testSignVerify() {
  const subtle = window.crypto.subtle;
  assert(subtle);
  for (const algorithm of ["RSA-PSS", "RSASSA-PKCS1-v1_5", "RSA-OAEP"]) {
    for (
      const hash of [
        "SHA-1",
        "SHA-256",
        "SHA-384",
        "SHA-512",
      ]
    ) {
      const keyUsages: KeyUsage[] = algorithm == "RSA-OAEP"
        ? ["encrypt", "decrypt"]
        : ["sign", "verify"];
      const keyPair = await subtle.generateKey(
        {
          name: algorithm,
          modulusLength: 2048,
          publicExponent: new Uint8Array([1, 0, 1]),
          hash,
        },
        true,
        keyUsages,
      );

      const data = new Uint8Array([1, 2, 3]);
      if (algorithm == "RSA-OAEP") {
        const encryptAlgorithm = { name: algorithm };
        const encypted = await subtle.encrypt(
          encryptAlgorithm,
          keyPair.publicKey,
          data,
        );

        assert(encypted);
        assert(encypted.byteLength > 0);
        assert(encypted instanceof ArrayBuffer);

        const decrypted = await subtle.decrypt(
          encryptAlgorithm,
          keyPair.privateKey,
          encypted,
        );
        assert(decrypted);
        assert(decrypted instanceof ArrayBuffer);
        assertEquals(new Uint8Array(decrypted), data);
      } else {
        const signAlgorithm = { name: algorithm, saltLength: 32 };

        const signature = await subtle.sign(
          signAlgorithm,
          keyPair.privateKey,
          data,
        );

        assert(signature);
        assert(signature.byteLength > 0);
        assert(signature.byteLength % 8 == 0);
        assert(signature instanceof ArrayBuffer);

        const verified = await subtle.verify(
          signAlgorithm,
          keyPair.publicKey,
          signature,
          data,
        );
        assert(verified);
      }
    }
  }
});

// TODO(@littledivy): Remove this when we enable WPT for encrypt_decrypt
unitTest(async function testEncryptDecrypt() {
  const subtle = window.crypto.subtle;
  assert(subtle);
  for (
    const hash of [
      "SHA-1",
      "SHA-256",
      "SHA-384",
      "SHA-512",
    ]
  ) {
    const keyPair = await subtle.generateKey(
      {
        name: "RSA-OAEP",
        modulusLength: 2048,
        publicExponent: new Uint8Array([1, 0, 1]),
        hash,
      },
      true,
      ["encrypt", "decrypt"],
    );

    const data = new Uint8Array([1, 2, 3]);
    const encryptAlgorithm = { name: "RSA-OAEP" };
    const encypted = await subtle.encrypt(
      encryptAlgorithm,
      keyPair.publicKey,
      data,
    );

    assert(encypted);
    assert(encypted.byteLength > 0);
    assert(encypted instanceof ArrayBuffer);

    const decrypted = await subtle.decrypt(
      encryptAlgorithm,
      keyPair.privateKey,
      encypted,
    );
    assert(decrypted);
    assert(decrypted instanceof ArrayBuffer);
    assertEquals(new Uint8Array(decrypted), data);
  }
});

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
      hash: "SHA-512",
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

// https://github.com/denoland/deno/issues/11313
unitTest(async function testSignRSASSAKey() {
  const subtle = window.crypto.subtle;
  assert(subtle);

  const keyPair = await subtle.generateKey(
    {
      name: "RSASSA-PKCS1-v1_5",
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

  const encoder = new TextEncoder();
  const encoded = encoder.encode("Hello, World!");

  const signature = await window.crypto.subtle.sign(
    { name: "RSASSA-PKCS1-v1_5" },
    keyPair.privateKey,
    encoded,
  );

  assert(signature);
});
