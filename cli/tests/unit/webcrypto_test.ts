import { assert, assertEquals, assertRejects, unitTest } from "./test_util.ts";

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
  for (const algorithm of ["RSA-PSS", "RSASSA-PKCS1-v1_5"]) {
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
          name: algorithm,
          modulusLength: 2048,
          publicExponent: new Uint8Array([1, 0, 1]),
          hash,
        },
        true,
        ["sign", "verify"],
      );

      const data = new Uint8Array([1, 2, 3]);

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
});

// deno-fmt-ignore
const plainText = new Uint8Array([95, 77, 186, 79, 50, 12, 12, 232, 118, 114, 90, 252, 229, 251, 210, 91, 248, 62, 90, 113, 37, 160, 140, 175, 231, 60, 62, 186, 196, 33, 119, 157, 249, 213, 93, 24, 12, 58, 233, 148, 38, 69, 225, 216, 47, 238, 140, 157, 41, 75, 60, 177, 160, 138, 153, 49, 32, 27, 60, 14, 129, 252, 71, 202, 207, 131, 21, 162, 175, 102, 50, 65, 19, 195, 182, 98, 48, 195, 70, 8, 196, 244, 89, 54, 52, 206, 2, 178, 103, 54, 34, 119, 240, 168, 64, 202, 116, 188, 61, 26, 98, 54, 149, 44, 94, 215, 170, 248, 168, 254, 203, 221, 250, 117, 132, 230, 151, 140, 234, 93, 42, 91, 159, 183, 241, 180, 140, 139, 11, 229, 138, 48, 82, 2, 117, 77, 131, 118, 16, 115, 116, 121, 60, 240, 38, 170, 238, 83, 0, 114, 125, 131, 108, 215, 30, 113, 179, 69, 221, 178, 228, 68, 70, 255, 197, 185, 1, 99, 84, 19, 137, 13, 145, 14, 163, 128, 152, 74, 144, 25, 16, 49, 50, 63, 22, 219, 204, 157, 107, 225, 104, 184, 72, 133, 56, 76, 160, 62, 18, 96, 10, 193, 194, 72, 2, 138, 243, 114, 108, 201, 52, 99, 136, 46, 168, 192, 42, 171]);

// Passing
const hashPlainTextVector = [
  {
    hash: "SHA-1",
    plainText: plainText.slice(0, 214),
  },
  {
    hash: "SHA-256",
    plainText: plainText.slice(0, 190),
  },
  {
    hash: "SHA-384",
    plainText: plainText.slice(0, 158),
  },
  {
    hash: "SHA-512",
    plainText: plainText.slice(0, 126),
  },
];

// TODO(@littledivy): Remove this when we enable WPT for encrypt_decrypt
unitTest(async function testEncryptDecrypt() {
  const subtle = window.crypto.subtle;
  assert(subtle);
  for (
    const { hash, plainText } of hashPlainTextVector
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

    const encryptAlgorithm = { name: "RSA-OAEP" };
    const cipherText = await subtle.encrypt(
      encryptAlgorithm,
      keyPair.publicKey,
      plainText,
    );

    assert(cipherText);
    assert(cipherText.byteLength > 0);
    assertEquals(cipherText.byteLength * 8, 2048);
    assert(cipherText instanceof ArrayBuffer);

    const decrypted = await subtle.decrypt(
      encryptAlgorithm,
      keyPair.privateKey,
      cipherText,
    );
    assert(decrypted);
    assert(decrypted instanceof ArrayBuffer);
    assertEquals(new Uint8Array(decrypted), plainText);

    const badPlainText = new Uint8Array(plainText.byteLength + 1);
    badPlainText.set(plainText, 0);
    badPlainText.set(new Uint8Array([32]), plainText.byteLength);
    await assertRejects(async () => {
      // Should fail
      await subtle.encrypt(
        encryptAlgorithm,
        keyPair.publicKey,
        badPlainText,
      );
      throw new TypeError("unreachable");
    }, DOMException);
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

unitTest(async function testECDSASignVerify() {
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
  assert(signature instanceof ArrayBuffer);

  const verified = await window.crypto.subtle.verify(
    { hash: { name: "SHA-384" }, name: "ECDSA" },
    key.publicKey,
    signature,
    encoded,
  );
  assert(verified);
});

// Tests the "bad paths" as a temporary replacement for sign_verify/ecdsa WPT.
unitTest(async function testECDSASignVerifyFail() {
  const key = await window.crypto.subtle.generateKey(
    {
      name: "ECDSA",
      namedCurve: "P-384",
    },
    true,
    ["sign", "verify"],
  );

  const encoded = new Uint8Array([1]);
  // Signing with a public key (InvalidAccessError)
  await assertRejects(async () => {
    await window.crypto.subtle.sign(
      { name: "ECDSA", hash: "SHA-384" },
      key.publicKey,
      new Uint8Array([1]),
    );
    throw new TypeError("unreachable");
  }, DOMException);

  // Do a valid sign for later verifying.
  const signature = await window.crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-384" },
    key.privateKey,
    encoded,
  );

  // Verifying with a private key (InvalidAccessError)
  await assertRejects(async () => {
    await window.crypto.subtle.verify(
      { hash: { name: "SHA-384" }, name: "ECDSA" },
      key.privateKey,
      signature,
      encoded,
    );
    throw new TypeError("unreachable");
  }, DOMException);
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

// deno-fmt-ignore
const rawKey = new Uint8Array([
  1, 2, 3, 4, 5, 6, 7, 8,
  9, 10, 11, 12, 13, 14, 15, 16
]);

const jwk: JsonWebKey = {
  kty: "oct",
  // unpadded base64 for rawKey.
  k: "AQIDBAUGBwgJCgsMDQ4PEA",
  alg: "HS256",
  ext: true,
  "key_ops": ["sign"],
};

unitTest(async function subtleCryptoHmacImportExport() {
  const key1 = await crypto.subtle.importKey(
    "raw",
    rawKey,
    { name: "HMAC", hash: "SHA-256" },
    true,
    ["sign"],
  );
  const key2 = await crypto.subtle.importKey(
    "jwk",
    jwk,
    { name: "HMAC", hash: "SHA-256" },
    true,
    ["sign"],
  );
  const actual1 = await crypto.subtle.sign(
    { name: "HMAC" },
    key1,
    new Uint8Array([1, 2, 3, 4]),
  );

  const actual2 = await crypto.subtle.sign(
    { name: "HMAC" },
    key2,
    new Uint8Array([1, 2, 3, 4]),
  );
  // deno-fmt-ignore
  const expected = new Uint8Array([
    59, 170, 255, 216, 51, 141, 51, 194,
    213, 48, 41, 191, 184, 40, 216, 47,
    130, 165, 203, 26, 163, 43, 38, 71,
    23, 122, 222, 1, 146, 46, 182, 87,
  ]);
  assertEquals(
    new Uint8Array(actual1),
    expected,
  );
  assertEquals(
    new Uint8Array(actual2),
    expected,
  );

  const exportedKey1 = await crypto.subtle.exportKey("raw", key1);
  assertEquals(new Uint8Array(exportedKey1), rawKey);

  const exportedKey2 = await crypto.subtle.exportKey("jwk", key2);
  assertEquals(exportedKey2, jwk);
});

// https://github.com/denoland/deno/issues/12085
unitTest(async function generateImportHmacJwk() {
  const key = await crypto.subtle.generateKey(
    {
      name: "HMAC",
      hash: "SHA-512",
    },
    true,
    ["sign"],
  );
  assert(key);
  assertEquals(key.type, "secret");
  assertEquals(key.extractable, true);
  assertEquals(key.usages, ["sign"]);

  const exportedKey = await crypto.subtle.exportKey("jwk", key);
  assertEquals(exportedKey.kty, "oct");
  assertEquals(exportedKey.alg, "HS512");
  assertEquals(exportedKey.key_ops, ["sign"]);
  assertEquals(exportedKey.ext, true);
  assert(typeof exportedKey.k == "string");
  assertEquals(exportedKey.k.length, 171);
});

// 2048-bits publicExponent=65537
const pkcs8TestVectors = [
  // rsaEncryption
  { pem: "cli/tests/testdata/webcrypto/id_rsaEncryption.pem", hash: "SHA-256" },
  // id-RSASSA-PSS (sha256)
  // `openssl genpkey -algorithm rsa-pss -pkeyopt rsa_pss_keygen_md:sha256 -out id_rsassaPss.pem`
  { pem: "cli/tests/testdata/webcrypto/id_rsassaPss.pem", hash: "SHA-256" },
  // id-RSASSA-PSS (default parameters)
  // `openssl genpkey -algorithm rsa-pss -out id_rsassaPss.pem`
  {
    pem: "cli/tests/testdata/webcrypto/id_rsassaPss_default.pem",
    hash: "SHA-1",
  },
  // id-RSASSA-PSS (default hash)
  // `openssl genpkey -algorithm rsa-pss -pkeyopt rsa_pss_keygen_saltlen:30 -out rsaPss_saltLen_30.pem`
  {
    pem: "cli/tests/testdata/webcrypto/id_rsassaPss_saltLen_30.pem",
    hash: "SHA-1",
  },
];

unitTest({ permissions: { read: true } }, async function importRsaPkcs8() {
  const pemHeader = "-----BEGIN PRIVATE KEY-----";
  const pemFooter = "-----END PRIVATE KEY-----";
  for (const { pem, hash } of pkcs8TestVectors) {
    const keyFile = await Deno.readTextFile(pem);
    const pemContents = keyFile.substring(
      pemHeader.length,
      keyFile.length - pemFooter.length,
    );
    const binaryDerString = atob(pemContents);
    const binaryDer = new Uint8Array(binaryDerString.length);
    for (let i = 0; i < binaryDerString.length; i++) {
      binaryDer[i] = binaryDerString.charCodeAt(i);
    }

    const key = await crypto.subtle.importKey(
      "pkcs8",
      binaryDer,
      { name: "RSA-PSS", hash },
      true,
      ["sign"],
    );

    assert(key);
    assertEquals(key.type, "private");
    assertEquals(key.extractable, true);
    assertEquals(key.usages, ["sign"]);
    const algorithm = key.algorithm as RsaHashedKeyAlgorithm;
    assertEquals(algorithm.name, "RSA-PSS");
    assertEquals(algorithm.hash.name, hash);
    assertEquals(algorithm.modulusLength, 2048);
    assertEquals(algorithm.publicExponent, new Uint8Array([1, 0, 1]));
  }
});

// deno-fmt-ignore
const asn1AlgorithmIdentifier = new Uint8Array([
  0x02, 0x01, 0x00, // INTEGER
  0x30, 0x0d, // SEQUENCE (2 elements)
  0x06, 0x09, // OBJECT IDENTIFIER
  0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, // 1.2.840.113549.1.1.1 (rsaEncryption)
  0x05, 0x00, // NULL
]);

unitTest(async function rsaExport() {
  for (const algorithm of ["RSASSA-PKCS1-v1_5", "RSA-PSS", "RSA-OAEP"]) {
    const keyPair = await crypto.subtle.generateKey(
      {
        name: algorithm,
        modulusLength: 2048,
        publicExponent: new Uint8Array([1, 0, 1]),
        hash: "SHA-256",
      },
      true,
      algorithm !== "RSA-OAEP" ? ["sign", "verify"] : ["encrypt", "decrypt"],
    );

    assert(keyPair.privateKey);
    assert(keyPair.publicKey);
    assertEquals(keyPair.privateKey.extractable, true);

    const exportedPrivateKey = await crypto.subtle.exportKey(
      "pkcs8",
      keyPair.privateKey,
    );

    assert(exportedPrivateKey);
    assert(exportedPrivateKey instanceof ArrayBuffer);

    const pkcs8 = new Uint8Array(exportedPrivateKey);
    assert(pkcs8.length > 0);

    assertEquals(
      pkcs8.slice(4, asn1AlgorithmIdentifier.byteLength + 4),
      asn1AlgorithmIdentifier,
    );

    const exportedPublicKey = await crypto.subtle.exportKey(
      "spki",
      keyPair.publicKey,
    );

    const spki = new Uint8Array(exportedPublicKey);
    assert(spki.length > 0);

    assertEquals(
      spki.slice(4, asn1AlgorithmIdentifier.byteLength + 1),
      asn1AlgorithmIdentifier.slice(3),
    );
  }
});

unitTest(async function testHkdfDeriveBits() {
  const rawKey = await crypto.getRandomValues(new Uint8Array(16));
  const key = await crypto.subtle.importKey(
    "raw",
    rawKey,
    { name: "HKDF", hash: "SHA-256" },
    false,
    ["deriveBits"],
  );
  const salt = await crypto.getRandomValues(new Uint8Array(16));
  const info = await crypto.getRandomValues(new Uint8Array(16));
  const result = await crypto.subtle.deriveBits(
    {
      name: "HKDF",
      hash: "SHA-256",
      salt: salt,
      info: info,
    },
    key,
    128,
  );
  assertEquals(result.byteLength, 128 / 8);
});

unitTest(async function testWrapKey() {
  // Test wrapKey
  const key = await crypto.subtle.generateKey(
    {
      name: "RSA-OAEP",
      modulusLength: 4096,
      publicExponent: new Uint8Array([1, 0, 1]),
      hash: "SHA-256",
    },
    true,
    ["wrapKey", "unwrapKey"],
  );

  const hmacKey = await crypto.subtle.generateKey(
    {
      name: "HMAC",
      hash: "SHA-256",
      length: 128,
    },
    true,
    ["sign"],
  );

  const wrappedKey = await crypto.subtle.wrapKey(
    "raw",
    hmacKey,
    key.publicKey,
    {
      name: "RSA-OAEP",
      label: new Uint8Array(8),
    },
  );

  assert(wrappedKey instanceof ArrayBuffer);
  assertEquals(wrappedKey.byteLength, 512);
});

// Doesn't need to cover all cases.
// Only for testing types.
unitTest(async function testAesKeyGen() {
  const key = await crypto.subtle.generateKey(
    {
      name: "AES-GCM",
      length: 256,
    },
    true,
    ["encrypt", "decrypt"],
  );

  assert(key);
  assertEquals(key.type, "secret");
  assertEquals(key.extractable, true);
  assertEquals(key.usages, ["encrypt", "decrypt"]);
  const algorithm = key.algorithm as AesKeyAlgorithm;
  assertEquals(algorithm.name, "AES-GCM");
  assertEquals(algorithm.length, 256);
});
