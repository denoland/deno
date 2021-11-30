import { assert, assertEquals, assertRejects } from "./test_util.ts";

// https://github.com/denoland/deno/issues/11664
Deno.test(async function testImportArrayBufferKey() {
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
Deno.test(async function testSignVerify() {
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
Deno.test(async function testEncryptDecrypt() {
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

Deno.test(async function testGenerateRSAKey() {
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

Deno.test(async function testGenerateHMACKey() {
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

Deno.test(async function testECDSASignVerify() {
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
Deno.test(async function testECDSASignVerifyFail() {
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
Deno.test(async function testSignRSASSAKey() {
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

Deno.test(async function subtleCryptoHmacImportExport() {
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
Deno.test(async function generateImportHmacJwk() {
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

Deno.test({ permissions: { read: true } }, async function importRsaPkcs8() {
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

Deno.test(async function rsaExport() {
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

Deno.test(async function testHkdfDeriveBits() {
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

Deno.test(async function testHkdfDeriveBitsWithLargeKeySize() {
  const key = await crypto.subtle.importKey(
    "raw",
    new Uint8Array([0x00]),
    "HKDF",
    false,
    ["deriveBits"],
  );
  await assertRejects(
    () =>
      crypto.subtle.deriveBits(
        {
          name: "HKDF",
          hash: "SHA-1",
          salt: new Uint8Array(),
          info: new Uint8Array(),
        },
        key,
        ((20 * 255) << 3) + 8,
      ),
    DOMException,
    "The length provided for HKDF is too large",
  );
});

Deno.test(async function testDeriveKey() {
  // Test deriveKey
  const rawKey = await crypto.getRandomValues(new Uint8Array(16));
  const key = await crypto.subtle.importKey(
    "raw",
    rawKey,
    "PBKDF2",
    false,
    ["deriveKey", "deriveBits"],
  );

  const salt = await crypto.getRandomValues(new Uint8Array(16));
  const derivedKey = await crypto.subtle.deriveKey(
    {
      name: "PBKDF2",
      salt,
      iterations: 1000,
      hash: "SHA-256",
    },
    key,
    { name: "HMAC", hash: "SHA-256" },
    true,
    ["sign"],
  );

  assert(derivedKey instanceof CryptoKey);
  assertEquals(derivedKey.type, "secret");
  assertEquals(derivedKey.extractable, true);
  assertEquals(derivedKey.usages, ["sign"]);

  const algorithm = derivedKey.algorithm as HmacKeyAlgorithm;
  assertEquals(algorithm.name, "HMAC");
  assertEquals(algorithm.hash.name, "SHA-256");
  assertEquals(algorithm.length, 256);
});

Deno.test(async function testAesCbcEncryptDecrypt() {
  const key = await crypto.subtle.generateKey(
    { name: "AES-CBC", length: 128 },
    true,
    ["encrypt", "decrypt"],
  );

  const iv = await crypto.getRandomValues(new Uint8Array(16));
  const encrypted = await crypto.subtle.encrypt(
    {
      name: "AES-CBC",
      iv,
    },
    key as CryptoKey,
    new Uint8Array([1, 2, 3, 4, 5, 6]),
  );

  assert(encrypted instanceof ArrayBuffer);
  assertEquals(encrypted.byteLength, 16);

  const decrypted = await crypto.subtle.decrypt(
    {
      name: "AES-CBC",
      iv,
    },
    key as CryptoKey,
    encrypted,
  );

  assert(decrypted instanceof ArrayBuffer);
  assertEquals(decrypted.byteLength, 6);
  assertEquals(new Uint8Array(decrypted), new Uint8Array([1, 2, 3, 4, 5, 6]));
});

// TODO(@littledivy): Enable WPT when we have importKey support
Deno.test(async function testECDH() {
  const namedCurve = "P-256";
  const keyPair = await crypto.subtle.generateKey(
    {
      name: "ECDH",
      namedCurve,
    },
    true,
    ["deriveBits"],
  );

  const derivedKey = await crypto.subtle.deriveBits(
    {
      name: "ECDH",
      public: keyPair.publicKey,
    },
    keyPair.privateKey,
    256,
  );

  assert(derivedKey instanceof ArrayBuffer);
  assertEquals(derivedKey.byteLength, 256 / 8);
});

Deno.test(async function testWrapKey() {
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
Deno.test(async function testAesKeyGen() {
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

Deno.test(async function testUnwrapKey() {
  // deno-fmt-ignore
  const salt = new Uint8Array([180,253,62,216,47,35,90,55,218,233,103,10,172,143,161,177]);
  const keyMaterial = await crypto.subtle.importKey(
    "raw",
    new Uint8Array([1, 2, 3]),
    { name: "PBKDF2" },
    false,
    ["deriveBits", "deriveKey"],
  );

  const unwrappingKey = await crypto.subtle.deriveKey(
    {
      "name": "PBKDF2",
      salt,
      "iterations": 100000,
      "hash": "SHA-256",
    },
    keyMaterial,
    { "name": "AES-CBC", "length": 256 },
    true,
    ["wrapKey", "unwrapKey", "decrypt"],
  );

  // deno-fmt-ignore
  const ivBytes = new Uint8Array([212,187,26,247,172,51,37,151,27,177,249,142]);

  // deno-fmt-ignore
  const wrappedKeyBuffer = new Uint8Array([6,155,182,208,7,141,44,18,3,151,58,126,68,100,252,
    225,241,11,25,201,153,171,102,174,150,29,62,195,110,138,106,109,14,6,108,
    148,104,221,22,93,102,221,146,25,65,112,4,140,79,194,164,163,156,250,108,
    11,14,220,78,194,161,17,14,57,121,70,13,28,220,210,78,32,46,217,36,165,220,
    170,244,152,214,150,83,2,138,128,11,251,227,213,72,100,158,10,162,40,195,
    60,248,77,37,156,34,10,213,171,67,147,73,231,31,63,80,176,103,206,187,164,
    214,250,49,223,185,5,48,241,17,1,253,59,185,181,209,255,42,223,175,90,159,
    174,169,205,156,120,195,1,135,165,226,46,119,27,97,183,23,197,227,85,138,
    235,79,158,167,59,62,194,34,210,214,240,215,101,233,63,138,53,87,253,189,
    27,66,150,76,242,76,102,174,179,163,184,205,11,161,224,19,110,34,175,192,
    101,117,169,86,66,56,241,128,13,156,165,125,139,110,138,50,108,129,251,137,
    26,186,110,117,113,207,179,59,213,18,175,14,203,192,2,97,131,125,167,227,
    182,87,72,123,54,156,60,195,88,224,96,46,126,245,251,247,147,110,147,173,
    82,106,93,210,55,71,127,133,41,37,181,17,106,16,158,220,136,43,75,133,96,
    240,151,116,40,44,254,2,32,74,226,193,172,48,211,71,109,163,143,30,92,28,
    30,183,25,16,176,207,77,93,139,242,114,91,218,126,123,234,18,9,245,53,46,
    172,215,62,92,249,191,17,27,0,58,151,33,23,169,93,177,253,152,147,198,196,
    226,42,202,166,99,250,127,40,221,196,121,195,198,235,30,159,159,95,182,107,
    175,137,177,49,72,63,131,162,198,186,22,255,230,237,195,56,147,177,101,52,
    227,125,32,180,242,47,92,212,6,148,218,107,125,137,123,15,51,107,159,228,
    238,212,60,54,184,48,110,248,252,208,46,23,149,78,169,201,68,242,193,251,
    156,227,42,90,109,102,172,61,207,124,96,98,79,37,218,16,212,139,162,0,183,
    235,171,75,18,84,160,120,173,156,187,99,24,58,88,213,148,24,193,111,75,169,
    10,158,207,148,84,249,156,248,19,221,2,175,1,8,74,221,212,244,123,34,223,
    175,54,166,101,51,175,141,80,87,9,146,72,223,46,251,199,192,2,22,125,16,15,
    99,26,159,165,133,172,169,26,236,44,86,182,162,81,143,249,15,207,12,232,15,
    205,199,78,133,199,19,232,183,33,183,72,117,72,27,43,254,13,17,252,1,143,
    137,154,10,4,77,85,24,85,143,200,81,76,171,43,124,42,191,150,70,10,90,178,
    198,40,233,233,225,146,231,209,254,2,90,216,5,97,105,204,82,88,81,99,92,
    159,116,192,223,148,252,12,24,197,211,187,212,98,252,201,154,184,65,54,47,
    13,106,151,168,208,112,212,74,204,36,233,98,104,58,103,1,194,13,26,109,101,
    60,42,3,215,20,25,99,176,63,28,112,102,121,190,96,198,228,196,78,38,82,37,
    248,42,150,115,6,10,22,101,42,237,175,69,232,212,231,40,193,70,211,245,106,
    231,175,150,88,105,170,139,238,196,64,218,250,47,165,22,36,196,161,30,79,
    175,14,133,88,129,182,56,140,147,168,134,91,68,172,110,195,134,156,68,78,
    249,215,68,250,11,23,70,59,156,99,75,249,159,84,16,206,93,16,130,34,66,210,
    82,252,53,251,84,59,226,212,154,15,20,163,58,228,109,53,214,151,237,10,169,
    107,180,123,174,159,182,8,240,115,115,220,131,128,79,80,61,133,58,24,98,
    193,225,56,36,159,254,199,49,44,160,28,81,140,163,24,143,114,31,237,235,
    250,83,72,215,44,232,182,45,39,182,193,248,65,174,186,52,219,30,198,48,1,
    134,151,81,114,38,124,7,213,205,138,28,22,216,76,46,224,241,88,156,7,62,
    23,104,34,54,25,156,93,212,133,182,61,93,255,195,68,244,234,53,132,151,140,
    72,146,127,113,227,34,243,218,222,47,218,113,18,173,203,158,133,90,156,214,
    77,20,113,1,231,164,52,55,69,132,24,68,131,212,7,153,34,179,113,156,81,
    127,83,57,29,195,90,64,211,115,202,188,5,42,188,142,203,109,231,53,206,72,
    220,90,23,12,1,178,122,60,221,68,6,14,154,108,203,171,142,159,249,13,55,52,
    110,214,33,147,164,181,50,79,164,200,83,251,40,105,223,50,0,115,240,146,23,
    122,80,204,169,38,198,154,31,29,23,236,39,35,131,147,242,163,138,158,236,
    117,7,108,33,132,98,50,111,46,146,251,82,34,85,5,130,237,67,40,170,235,124,
    92,66,71,239,12,97,136,251,1,206,13,51,232,92,46,35,95,5,123,24,183,99,243,
    124,75,155,89,66,54,72,17,255,99,137,199,232,204,9,248,78,35,218,136,117,
    239,102,240,187,40,89,244,140,109,229,120,116,54,207,171,11,248,190,199,81,
    53,109,8,188,51,93,165,34,255,165,191,198,130,220,41,192,166,194,69,104,
    124,158,122,236,176,24,60,87,240,42,158,143,37,143,208,155,249,230,21,4,
    230,56,194,62,235,132,14,50,180,216,134,28,25,159,64,199,161,236,60,233,
    160,172,68,169,2,5,252,190,20,54,115,248,63,93,107,156,8,96,85,32,189,118,
    66,114,126,64,203,97,235,13,18,102,192,51,59,5,122,171,96,129,40,32,154,4,
    191,234,75,184,112,201,244,110,50,216,44,88,139,175,58,112,7,52,25,64,112,
    40,148,187,39,234,96,151,16,158,114,113,109,164,47,108,94,148,35,232,221,
    33,110,126,170,25,234,45,165,180,210,193,120,247,155,127]);

  await crypto.subtle.unwrapKey(
    "pkcs8",
    wrappedKeyBuffer,
    unwrappingKey,
    {
      name: "AES-CBC",
      iv: ivBytes,
    },
    {
      name: "RSA-PSS",
      hash: "SHA-256",
    },
    true,
    ["sign"],
  );
});

Deno.test(async function testDecryptWithInvalidIntializationVector() {
  const data = new Uint8Array([42, 42, 42, 42]);
  const key = await crypto.subtle.generateKey(
    { name: "AES-CBC", length: 256 },
    true,
    ["encrypt", "decrypt"],
  );
  const initVector = crypto.getRandomValues(new Uint8Array(16));
  const encrypted = await crypto.subtle.encrypt(
    { name: "AES-CBC", iv: initVector },
    key,
    data,
  );
  const initVector2 = crypto.getRandomValues(new Uint8Array(16));
  await assertRejects(async () => {
    await crypto.subtle.decrypt(
      { name: "AES-CBC", iv: initVector2 },
      key,
      encrypted,
    );
  }, DOMException);
});
