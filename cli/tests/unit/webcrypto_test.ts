import {
  assert,
  assertEquals,
  assertThrowsAsync,
  unitTest,
} from "./test_util.ts";

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
    await assertThrowsAsync(async () => {
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
  await assertThrowsAsync(async () => {
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
  await assertThrowsAsync(async () => {
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

const jwkRsaPublicPSS: JsonWebKey = {
  kty: "RSA",
  // unpadded base64 for rawKey.
  n: "09eVwAhT9SPBxdEN-74BBeEANGaVGwqH-YglIc4VV7jfhR2by5ivzVq8NCeQ1_ACDIlTDY8CTMQ5E1c1SEXmo_T7q84XUGXf8U9mx6uRg46sV7fF-hkwJR80BFVsvWxp4ahPlVJYj__94ft7rIVvchb5tyalOjrYFCJoFnSgq-i3ZjU06csI9XnO5klINucD_Qq0vUhO23_Add2HSYoRjab8YiJJR_Eths7Pq6HHd2RSXmwYp5foRnwe0_U75XmesHWDJlJUHYbwCZo0kP9G8g4QbucwU-MSNBkZOO2x2ZtZNexpHd0ThkATbnNlpVG_z2AGNORp_Ve3rlXwrGIXXw",
  e: "AQAB",
  alg: "PS256",
  ext: true,
  key_ops: ["verify"],
};

const jwkRsaPrivatePSS: JsonWebKey = {
  alg: "PS256",
  d: "H4xboN2co0VP9kXL71G8lUOM5EDis8Q9u8uqu_4U75t4rjpamVeD1vFMVfgOehokM_m_hKVnkkcmuNqj9L90ObaiRFPM5QxG7YkFpXbHlPAKeoXD1hsqMF0VQg_2wb8DhberInHA_rEA_kaVhHvavQLu7Xez45gf1d_J4I4931vjlCB6cupbLL0H5hHsxbMsX_5nnmAJdL_U3gD-U7ZdQheUPhDBJR2KeGzvnTm3KVKpOnwn-1Cd45MU4-KDdP0FcBVEuBsSrsQHliTaciBgkbyj__BangPj3edDxTkb-fKkEvhkXRjAoJs1ixt8nfSGDce9cM_GqAX9XGb4s2QkAQ",
  dp:
    "mM82RBwzGzi9LAqjGbi-badLtHRRBoH9sfMrJuOtzxRnmwBFccg_lwy-qAhUTqnN9kvD0H1FzXWzoFPFJbyi-AOmumYGpWm_PvzQGldne5CPJ02pYaeg-t1BePsT3OpIq0Am8E2Kjf9polpRJwIjO7Kx8UJKkhg5bISnsy0V8wE",
  dq:
    "ZlM4AvrWIpXwqsH_5Q-6BsLJdbnN_GypFCXoT9VXniXncSBZIWCkgDndBdWkSzyzIN65NiMRBfZaf9yduTFj4kvOPwb3ch3J0OxGJk0Ary4OGSlS1zNwMl93ALGal1FzpWUuiia9L9RraGqXAUr13L7TIIMRobRjpAV-z7M-ruM",
  e: "AQAB",
  ext: true,
  key_ops: ["sign"],
  kty: "RSA",
  n: "09eVwAhT9SPBxdEN-74BBeEANGaVGwqH-YglIc4VV7jfhR2by5ivzVq8NCeQ1_ACDIlTDY8CTMQ5E1c1SEXmo_T7q84XUGXf8U9mx6uRg46sV7fF-hkwJR80BFVsvWxp4ahPlVJYj__94ft7rIVvchb5tyalOjrYFCJoFnSgq-i3ZjU06csI9XnO5klINucD_Qq0vUhO23_Add2HSYoRjab8YiJJR_Eths7Pq6HHd2RSXmwYp5foRnwe0_U75XmesHWDJlJUHYbwCZo0kP9G8g4QbucwU-MSNBkZOO2x2ZtZNexpHd0ThkATbnNlpVG_z2AGNORp_Ve3rlXwrGIXXw",
  p: "7VwGt_tJcAFQHrmDw5dM1EBru6fidM45NDv6VVOEbxKuD5Sh2EfAHfm5c6oouA1gZqwvKH0sn_XpB1NsyYyHEQd3sBVdK0zRjTo-E9mRP-1s-LMd5YDXVq6HE339nxpXsmO25slQEF6zBrj1bSNNXBFc7fgDnlq-HIeleMvsY_E",
  q: "5HqMHLzb4IgXhUl4pLz7E4kjY8PH2YGzaQfK805zJMbOXzmlZK0hizKo34Qqd2nB9xos7QgzOYQrNfSWheARwVsSQzAE0vGvw3zHIPP_lTtChBlCTPctQcURjw4dXcnK1oQ-IT321FNOW3EO-YTsyGcypJqJujlZrLbxYjOjQE8",
  qi:
    "OQXzi9gypDnpdHatIi0FaUGP8LSzfVH0AUugURJXs4BTJpvA9y4hcpBQLrcl7H_vq6kbGmvC49V-9I5HNVX_AuxGIXKuLZr5WOxPq8gLTqHV7X5ZJDtWIP_nq2NNgCQQyNNRrxebiWlwGK9GnX_unewT6jopI_oFhwp0Q13rBR0",
};

const pkcs8RsaPrivatePss =
  "308204bd020100300d06092a864886f70d0101010500048204a7308204a30201000282010100d3d795c00853f523c1c5d10dfbbe0105e1003466951b0a87f9882521ce1557b8df851d9bcb98afcd5abc342790d7f0020c89530d8f024cc4391357354845e6a3f4fbabce175065dff14f66c7ab91838eac57b7c5fa1930251f3404556cbd6c69e1a84f9552588ffffde1fb7bac856f7216f9b726a53a3ad81422681674a0abe8b7663534e9cb08f579cee6494836e703fd0ab4bd484edb7fc075dd87498a118da6fc62224947f12d86cecfaba1c77764525e6c18a797e8467c1ed3f53be5799eb075832652541d86f0099a3490ff46f20e106ee73053e31234191938edb1d99b5935ec691ddd138640136e7365a551bfcf600634e469fd57b7ae55f0ac62175f0203010001028201001f8c5ba0dd9ca3454ff645cbef51bc95438ce440e2b3c43dbbcbaabbfe14ef9b78ae3a5a995783d6f14c55f80e7a1a2433f9bf84a567924726b8daa3f4bf7439b6a24453cce50c46ed8905a576c794f00a7a85c3d61b2a305d15420ff6c1bf0385b7ab2271c0feb100fe4695847bdabd02eeed77b3e3981fd5dfc9e08e3ddf5be394207a72ea5b2cbd07e611ecc5b32c5ffe679e600974bfd4de00fe53b65d4217943e10c1251d8a786cef9d39b72952a93a7c27fb509de39314e3e28374fd05701544b81b12aec4079624da72206091bca3fff05a9e03e3dde743c5391bf9f2a412f8645d18c0a09b358b1b7c9df4860dc7bd70cfc6a805fd5c66f8b364240102818100ed5c06b7fb497001501eb983c3974cd4406bbba7e274ce39343bfa5553846f12ae0f94a1d847c01df9b973aa28b80d6066ac2f287d2c9ff5e907536cc98c87110777b0155d2b4cd18d3a3e13d9913fed6cf8b31de580d756ae87137dfd9f1a57b263b6e6c950105eb306b8f56d234d5c115cedf8039e5abe1c87a578cbec63f102818100e47a8c1cbcdbe08817854978a4bcfb13892363c3c7d981b36907caf34e7324c6ce5f39a564ad218b32a8df842a7769c1f71a2ced083339842b35f49685e011c15b12433004d2f1afc37cc720f3ff953b428419424cf72d41c5118f0e1d5dc9cad6843e213df6d4534e5b710ef984ecc86732a49a89ba3959acb6f16233a3404f0281810098cf36441c331b38bd2c0aa319b8be6da74bb474510681fdb1f32b26e3adcf14679b004571c83f970cbea808544ea9cdf64bc3d07d45cd75b3a053c525bca2f803a6ba6606a569bf3efcd01a57677b908f274da961a7a0fadd4178fb13dcea48ab4026f04d8a8dff69a25a512702233bb2b1f1424a9218396c84a7b32d15f30102818066533802fad62295f0aac1ffe50fba06c2c975b9cdfc6ca91425e84fd5579e25e77120592160a48039dd05d5a44b3cb320deb936231105f65a7fdc9db93163e24bce3f06f7721dc9d0ec46264d00af2e0e192952d73370325f7700b19a975173a5652e8a26bd2fd46b686a97014af5dcbed3208311a1b463a4057ecfb33eaee30281803905f38bd832a439e97476ad222d0569418ff0b4b37d51f4014ba0511257b38053269bc0f72e217290502eb725ec7fefaba91b1a6bc2e3d57ef48e473555ff02ec462172ae2d9af958ec4fabc80b4ea1d5ed7e59243b5620ffe7ab634d802410c8d351af179b89697018af469d7fee9dec13ea3a2923fa05870a74435deb051d";

const spkiRsaPssPublic =
  "30820122300d06092a864886f70d01010105000382010f003082010a0282010100d3d795c00853f523c1c5d10dfbbe0105e1003466951b0a87f9882521ce1557b8df851d9bcb98afcd5abc342790d7f0020c89530d8f024cc4391357354845e6a3f4fbabce175065dff14f66c7ab91838eac57b7c5fa1930251f3404556cbd6c69e1a84f9552588ffffde1fb7bac856f7216f9b726a53a3ad81422681674a0abe8b7663534e9cb08f579cee6494836e703fd0ab4bd484edb7fc075dd87498a118da6fc62224947f12d86cecfaba1c77764525e6c18a797e8467c1ed3f53be5799eb075832652541d86f0099a3490ff46f20e106ee73053e31234191938edb1d99b5935ec691ddd138640136e7365a551bfcf600634e469fd57b7ae55f0ac62175f0203010001";

function hexToUint8Array(hex: string) {
  if (!(/[0-9A-Fa-f]{1,2}/g).test(hex)) {
    throw new Error("Invalid hexadecimal");
  }

  return Uint8Array.from(
    hex.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? [],
  );
}
unitTest(async function subtleCryptoRsaPssImportExport() {
  const keyPriv1 = await crypto.subtle.importKey(
    "jwk",
    jwkRsaPrivatePSS,
    { name: "RSA-PSS", hash: "SHA-256" },
    true,
    ["sign"],
  );
  const keyPub1 = await crypto.subtle.importKey(
    "jwk",
    jwkRsaPublicPSS,
    { name: "RSA-PSS", hash: "SHA-256" },
    true,
    ["verify"],
  );
  const signature1 = await crypto.subtle.sign(
    { name: "RSA-PSS", saltLength: 32 },
    keyPriv1,
    new Uint8Array([1, 2, 3, 4]),
  );

  let verifyOK = await crypto.subtle.verify(
    { name: "RSA-PSS", saltLength: 32 },
    keyPub1,
    signature1,
    new Uint8Array([1, 2, 3, 4]),
  );
  assert(verifyOK);

  /* TODO(@seanwykes) - implement RSA-PSS pkcs8 + spki*/
  const keyPriv2 = await crypto.subtle.importKey(
    "pkcs8",
    hexToUint8Array(pkcs8RsaPrivatePss),
    { name: "RSA-PSS", hash: "SHA-256" },
    true,
    ["sign"],
  );

  const keyPub2 = await crypto.subtle.importKey(
    "spki",
    hexToUint8Array(spkiRsaPssPublic),
    { name: "RSA-PSS", hash: "SHA-256" },
    true,
    ["verify"],
  );

  verifyOK = await crypto.subtle.verify(
    { name: "RSA-PSS", saltLength: 32 },
    keyPub2,
    signature1,
    new Uint8Array([1, 2, 3, 4]),
  );
  assert(verifyOK);

  const signature2 = await crypto.subtle.sign(
    { name: "RSA-PSS", saltLength: 32 },
    keyPriv2,
    new Uint8Array([1, 2, 3, 4]),
  );

  verifyOK = await crypto.subtle.verify(
    { name: "RSA-PSS", saltLength: 32 },
    keyPub2,
    signature2,
    new Uint8Array([1, 2, 3, 4]),
  );
  assert(verifyOK);

  verifyOK = await crypto.subtle.verify(
    { name: "RSA-PSS", saltLength: 32 },
    keyPub1,
    signature2,
    new Uint8Array([1, 2, 3, 4]),
  );
  assert(verifyOK);
});
