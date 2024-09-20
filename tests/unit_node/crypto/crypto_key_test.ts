// deno-lint-ignore-file no-explicit-any

// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  createECDH,
  createHmac,
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  createSign,
  generateKeyPair,
  generateKeyPairSync,
  KeyObject,
  randomBytes,
} from "node:crypto";
import { promisify } from "node:util";
import { Buffer } from "node:buffer";
import { assert, assertEquals, assertThrows } from "@std/assert";

const RUN_SLOW_TESTS = Deno.env.get("SLOW_TESTS") === "1";

const generateKeyPairAsync = promisify(
  (
    type: any,
    options: any,
    callback: (
      err: Error | null,
      key: { publicKey: KeyObject; privateKey: KeyObject },
    ) => void,
  ) =>
    generateKeyPair(
      type,
      options,
      (err: Error | null, publicKey: KeyObject, privateKey: KeyObject) => {
        callback(err, { publicKey, privateKey });
      },
    ),
);

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

const modulusLengths = RUN_SLOW_TESTS ? [2048, 3072] : [2048];

for (const type of ["rsa", "rsa-pss", "dsa"]) {
  for (const modulusLength of modulusLengths) {
    Deno.test({
      name: `generate ${type} key ${modulusLength}`,
      fn() {
        const { publicKey, privateKey } = generateKeyPairSync(type as any, {
          modulusLength,
        });

        assertEquals(publicKey.type, "public");
        assertEquals(privateKey.type, "private");
      },
    });

    Deno.test({
      name: `generate ${type} key async ${modulusLength}`,
      async fn() {
        const x = await generateKeyPairAsync(type as any, {
          modulusLength,
        });
        const { publicKey, privateKey } = x;
        assertEquals(publicKey.type, "public");
        assertEquals(privateKey.type, "private");
      },
    });
  }
}

for (
  const namedCurve of [
    "P-384",
    "prime384v1",
    "secp384r1",
    "P-256",
    "prime256v1",
    "secp256r1",
  ]
) {
  Deno.test({
    name: `generate ec key ${namedCurve}`,
    fn() {
      const { publicKey, privateKey } = generateKeyPairSync("ec", {
        namedCurve,
      });

      assertEquals(publicKey.type, "public");
      assertEquals(privateKey.type, "private");
    },
  });

  Deno.test({
    name: `generate ec key ${namedCurve} async`,
    async fn() {
      const { publicKey, privateKey } = await generateKeyPairAsync("ec", {
        namedCurve,
      });

      assertEquals(publicKey.type, "public");
      assertEquals(privateKey.type, "private");
    },
  });

  Deno.test({
    name: `generate ec key ${namedCurve} paramEncoding=explicit fails`,
    fn() {
      assertThrows(() => {
        // @ts-ignore: @types/node is broken?
        generateKeyPairSync("ec", {
          namedCurve,
          paramEncoding: "explicit",
        });
      });
    },
  });
}

for (
  const groupName of ["modp5", "modp14", "modp15", "modp16", "modp17", "modp18"]
) {
  Deno.test({
    name: `generate dh key ${groupName}`,
    fn() {
      // @ts-ignore: @types/node is broken?
      const { publicKey, privateKey } = generateKeyPairSync("dh", {
        group: groupName,
      });

      assertEquals(publicKey.type, "public");
      assertEquals(privateKey.type, "private");
    },
  });

  Deno.test({
    name: `generate dh key ${groupName} async`,
    async fn() {
      // @ts-ignore: @types/node is broken?
      const { publicKey, privateKey } = await generateKeyPairAsync("dh", {
        group: groupName,
      });

      assertEquals(publicKey.type, "public");
      assertEquals(privateKey.type, "private");
    },
  });
}

const primeLengths = RUN_SLOW_TESTS ? [1024, 2048, 4096] : [1024];

for (const primeLength of primeLengths) {
  Deno.test({
    name: `generate dh key ${primeLength}`,
    fn() {
      // @ts-ignore: @types/node is broken?
      const { publicKey, privateKey } = generateKeyPairSync("dh", {
        primeLength,
        generator: 2,
      });

      assertEquals(publicKey.type, "public");
      assertEquals(privateKey.type, "private");
    },
  });

  Deno.test({
    name: `generate dh key ${primeLength} async`,
    async fn() {
      // @ts-ignore: @types/node is broken?
      const { publicKey, privateKey } = await generateKeyPairAsync("dh", {
        primeLength,
        generator: 2,
      });

      assertEquals(publicKey.type, "public");
      assertEquals(privateKey.type, "private");
    },
  });
}

const rsaPrivateKey = Deno.readTextFileSync(
  new URL("../testdata/rsa_private.pem", import.meta.url),
);

Deno.test("createPrivateKey rsa", function () {
  const key = createPrivateKey(rsaPrivateKey);
  assertEquals(key.type, "private");
  assertEquals(key.asymmetricKeyType, "rsa");
  assertEquals(key.asymmetricKeyDetails?.modulusLength, 2048);
  assertEquals(key.asymmetricKeyDetails?.publicExponent, 65537n);
});

Deno.test("createPrivateKey dh", function () {
  // 1.2.840.113549.1.3.1
  const pem = "-----BEGIN PRIVATE KEY-----\n" +
    "MIIBoQIBADCB1QYJKoZIhvcNAQMBMIHHAoHBAP//////////yQ/aoiFowjTExmKL\n" +
    "gNwc0SkCTgiKZ8x0Agu+pjsTmyJRSgh5jjQE3e+VGbPNOkMbMCsKbfJfFDdP4TVt\n" +
    "bVHCReSFtXZiXn7G9ExC6aY37WsL/1y29Aa37e44a/taiZ+lrp8kEXxLH+ZJKGZR\n" +
    "7ORbPcIAfLihY78FmNpINhxV05ppFj+o/STPX4NlXSPco62WHGLzViCFUrue1SkH\n" +
    "cJaWbWcMNU5KvJgE8XRsCMojcyf//////////wIBAgSBwwKBwHxnT7Zw2Ehh1vyw\n" +
    "eolzQFHQzyuT0y+3BF+FxK2Ox7VPguTp57wQfGHbORJ2cwCdLx2mFM7gk4tZ6COS\n" +
    "E3Vta85a/PuhKXNLRdP79JgLnNtVtKXB+ePDS5C2GgXH1RHvqEdJh7JYnMy7Zj4P\n" +
    "GagGtIy3dV5f4FA0B/2C97jQ1pO16ah8gSLQRKsNpTCw2rqsZusE0rK6RaYAef7H\n" +
    "y/0tmLIsHxLIn+WK9CANqMbCWoP4I178BQaqhiOBkNyNZ0ndqA==\n" +
    "-----END PRIVATE KEY-----";
  const key = createPrivateKey(pem);
  assertEquals(key.type, "private");
  assertEquals(key.asymmetricKeyType, "dh");
});

Deno.test("createPublicKey dh", function () {
  // 1.2.840.113549.1.3.1
  const pem = "-----BEGIN PUBLIC KEY-----\n" +
    "MIIBnzCB1QYJKoZIhvcNAQMBMIHHAoHBAP//////////yQ/aoiFowjTExmKLgNwc\n" +
    "0SkCTgiKZ8x0Agu+pjsTmyJRSgh5jjQE3e+VGbPNOkMbMCsKbfJfFDdP4TVtbVHC\n" +
    "ReSFtXZiXn7G9ExC6aY37WsL/1y29Aa37e44a/taiZ+lrp8kEXxLH+ZJKGZR7ORb\n" +
    "PcIAfLihY78FmNpINhxV05ppFj+o/STPX4NlXSPco62WHGLzViCFUrue1SkHcJaW\n" +
    "bWcMNU5KvJgE8XRsCMojcyf//////////wIBAgOBxAACgcBR7+iL5qx7aOb9K+aZ\n" +
    "y2oLt7ST33sDKT+nxpag6cWDDWzPBKFDCJ8fr0v7yW453px8N4qi4R7SYYxFBaYN\n" +
    "Y3JvgDg1ct2JC9sxSuUOLqSFn3hpmAjW7cS0kExIVGfdLlYtIqbhhuo45cTEbVIM\n" +
    "rDEz8mjIlnvbWpKB9+uYmbjfVoc3leFvUBqfG2In2m23Md1swsPxr3n7g68H66JX\n" +
    "iBJKZLQMqNdbY14G9rdKmhhTJrQjC+i7Q/wI8JPhOFzHIGA=\n" +
    "-----END PUBLIC KEY-----";
  const key = createPublicKey(pem);
  assertEquals(key.type, "public");
  assertEquals(key.asymmetricKeyType, "dh");
});

// openssl ecparam -name secp256r1 -genkey -noout -out a.pem
// openssl pkcs8 -topk8 -nocrypt -in a.pem -out b.pem
const ecPrivateKey = Deno.readTextFileSync(
  new URL("./ec_private_secp256r1.pem", import.meta.url),
);

Deno.test("createPrivateKey ec", function () {
  const key = createPrivateKey(ecPrivateKey);
  assertEquals(key.type, "private");
  assertEquals(key.asymmetricKeyType, "ec");
  assertEquals(key.asymmetricKeyDetails?.namedCurve, "p256");
});

const rsaPublicKey = Deno.readTextFileSync(
  new URL("../testdata/rsa_public.pem", import.meta.url),
);

Deno.test("createPublicKey() RSA", () => {
  const key = createPublicKey(rsaPublicKey);
  assertEquals(key.type, "public");
  assertEquals(key.asymmetricKeyType, "rsa");
  assertEquals(key.asymmetricKeyDetails?.modulusLength, 2048);
  assertEquals(key.asymmetricKeyDetails?.publicExponent, 65537n);
});

// openssl ecparam -name prime256v1 -genkey -noout -out a.pem
// openssl ec -in a.pem -pubout -out b.pem
const ecPublicKey = Deno.readTextFileSync(
  new URL("../testdata/ec_prime256v1_public.pem", import.meta.url),
);

Deno.test("createPublicKey() EC", function () {
  const key = createPublicKey(ecPublicKey);
  assertEquals(key.type, "public");
  assertEquals(key.asymmetricKeyType, "ec");
  assertEquals(key.asymmetricKeyDetails?.namedCurve, "p256");
});

Deno.test("createPublicKey SPKI for DH", async function () {
  const { publicKey, privateKey } = await crypto.subtle.generateKey(
    {
      name: "ECDH",
      namedCurve: "P-384",
    },
    true,
    ["deriveKey", "deriveBits"],
  );

  const exportedPublicKey = await crypto.subtle.exportKey("spki", publicKey);
  const exportedPrivateKey = await crypto.subtle.exportKey("pkcs8", privateKey);

  const pubKey = createPublicKey({
    key: Buffer.from(exportedPublicKey),
    format: "der",
    type: "spki",
  });

  const privKey = createPrivateKey({
    key: Buffer.from(exportedPrivateKey),
    format: "der",
    type: "pkcs8",
  });

  assertEquals(pubKey.asymmetricKeyType, "ec");
  assertEquals(privKey.asymmetricKeyType, "ec");
});

Deno.test("ECDH generateKeys compressed", function () {
  const ecdh = createECDH("secp256k1");
  const publicKey = ecdh.generateKeys("binary", "compressed");
  assertEquals(publicKey.length, 33);

  const uncompressedKey = ecdh.generateKeys("binary");
  assertEquals(uncompressedKey.length, 65);
});

Deno.test("ECDH getPublicKey compressed", function () {
  const ecdh = createECDH("secp256k1");
  for (const format of ["compressed", "uncompressed"] as const) {
    ecdh.generateKeys("binary", format);

    const compressedKey = ecdh.getPublicKey("binary", "compressed");
    assertEquals(compressedKey.length, 33);

    const uncompressedKey = ecdh.getPublicKey("binary");
    assertEquals(uncompressedKey.length, 65);
  }
});

// https://github.com/denoland/deno/issues/20938
Deno.test("rsa jwt signer", function () {
  const token = `-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCVoKHiWLQKlnCG
oR4d8g+OSGXoJ3yY+BFubB+1TSaCvWGLkqHjYAA0UCgOoaazE2YnXhVlO4tLDLn/
5R6PJrsxksnup+AWnEbur+CuaeQqizGTQXA0nUrsza/QJxb05GSMW9kupzI5BXBi
1R8Tmo5I0CpmXDi1yF+nL2NeDXHB999tXcRSrN/Ai11G1HFoYVs36+cm/Jw71BB1
KsokwFuzvxTFL9bOXDy8/8MlF8QSBFaWBN1tNZ40+oLf/rYeNXpZAFywvC9rc/Ud
B7J9InYHJQaq+vzuWNq7l0LnkJK+/Mq3AYl5yStjBie6tXC3LCmQ5MmLdzHE+SBQ
7tUIL6fvAgMBAAECggEAMSMJRp2+smNpHK04iLj/ZshbvvuIrWt5vfuABjgJ15F9
wSosQ9E4//T60qM/bTuHppH9ELuXKNRLGMATYwtjGgqMifVTX9l+adAURvK7XUVM
yIEK6hxliJKblA3iOhXu9zEKh4mcsqEYoTw/8l4lL8A8zFSowvnEf9DOHwrnOr09
bV6+6BZbLgugLqtOB7i5agnviiCV4Z4llWdhP3zW3c8/PUQyTsqebTkY0DB4FnI0
vC0kQU/v/7MCueH0FA4fMEY9CWuzL3809I9rvUPIBgqSkpXEoWxoGUJxIHGYK6fG
+HHjZQp87Sfz5G4g/Qrq2Gqc2Mb7I0QS2zgBu1tx0QKBgQDH3EyxQ6W9r2S1WqEm
w2B32AuemWwIdxaeLf4est0rO0G0ihAsx4vNZElKO7weYDypp8AjeYfjuriweyQA
R8KDWonn9jA2QQfNNkXDIq+d5+zFbfdOFGqQEThLtpi5pPh0+NeUGQQZIb07jqLF
giuZgOmPVFwru8jYLO04GTZoEwKBgQC/qCP74LHI3/35Ftx5l9CnM5Zr84ByoI5B
3xt2Sd9OsxULxY/omvcB2EdBZSTVKunGmF2a7MDpOn0r/7FdSuuuqzrMwRqbzRFA
GSO06vnoA/k6llcfXKqLZqjHuHEAUNpEeAuzNUKP2DgvnHRtXSkBpFb+IUTMlL9y
O55+g570NQKBgBZiSgSgevOfrTvShrH8t9U0UTjWHg9kpxfYNtnhVnv9CwLZY65g
Ovwp+zthLVSJmsG1lANlHR8YTo8Ve5a8csCbZ06feA7bgbAuH+oW/GxHCXGjO0t3
Zef0xcVVEg3YuCsBo8NmedsGuFbvRrOwPInYsk/nNtt/EKOFhJv/1uQZAoGAdaYb
YLTPrcWCO/PCp4l/9gN+5Ql24eaZLOpuCzDRuZfI5Y8DBgCKfEFtjEEwWQKcuuIx
I7cNvJ3A1M+C6wfgoTpPP/2R/e3mFvjXqGlNuxUlWseK95+EuUntdZxNEaqQMdOX
Kw0YrQBHjUJ3XeMAFxfwptN5TjRJSTA73OGjI7kCgYBtw1LmjFd6wJFyqLEsBnqO
gnVnpxf1DMeMUFpNr+M14P3ETa5UaqiMvCD6VGOzCYv1F7JhnS9TFsYY+FV+L3Nh
1+bZIPY4D4seyPgH0cCycBSVfqdAHJlfxf/Pm7lHCNxTGEfBWri4Ga1bW+zQpWY7
SogaIHQjE81ZkmNtU5gM5Q==
-----END PRIVATE KEY-----`;

  const key = createPrivateKey(token);
  assertEquals(key.type, "private");
  assertEquals(key.asymmetricKeyType, "rsa");
  assertEquals(key.asymmetricKeyDetails?.modulusLength, 2048);
  assertEquals(key.asymmetricKeyDetails?.publicExponent, 65537n);

  const signer = createSign("RSA-SHA256");
  signer.update("hello");
  const signature = signer.sign(key, "base64");

  assertEquals(
    signature,
    `jEwckJ/d5GkF/8TTm+wllq2JNghG/m2JYJIW7vS8Vms53zCTTNSSegTSoIVoxWymwTPw2dTtZi41Lg0O271/WvEmQhiWD2dnjz6D/0F4eyn+QUhcmGCadDFyfp7+8x1XOppSw2YB8vL5WCL0QDdp3TAa/rWI0Hn4OftHMa6HPvatkGs+8XlQOGCCfd3TLg+t1UROgpgmetjoAM67mlwxXMGGu/Tr/EbXnnINKeB0iuSmD1FCxlrgFuYWDKxd79n2jZ74FrS/zto+bqWSI5uUa4Ar7yvXtek1Cu1OFM6vgdN9Y6Po2UD9+IT04EhU03LUDY5paYOO8yohz7p7kqHvpA==`,
  );
});

Deno.test("generate rsa export public key", async function () {
  const { publicKey } = await generateKeyPairAsync("rsa", {
    modulusLength: 2048,
  });

  const spkiPem = publicKey.export({ format: "pem", type: "spki" });
  assert(typeof spkiPem === "string");
  assert(spkiPem.startsWith("-----BEGIN PUBLIC KEY-----"));

  const der = publicKey.export({ format: "der", type: "spki" });
  assert(der instanceof Uint8Array);
});

Deno.test("create public key with invalid utf-8 string", function () {
  // This is an invalid UTF-8 string because it contains a lone utf-16 surrogate.
  const invalidPem = Buffer.from(new Uint8Array([0xE2, 0x28, 0xA1]));
  assertThrows(
    () => {
      createPublicKey(invalidPem);
    },
    Error,
    "not valid utf8",
  );
});

Deno.test("create private key with invalid utf-8 string", function () {
  // This is an invalid UTF-8 string because it contains a lone utf-16 surrogate.
  const invalidPem = Buffer.from(new Uint8Array([0xE2, 0x28, 0xA1]));
  assertThrows(
    () => {
      createPrivateKey(invalidPem);
    },
    Error,
    "not valid utf8",
  );
});

Deno.test("RSA JWK import public key", function () {
  const key = {
    "kty": "RSA",
    "alg": "RS256",
    "n":
      "5Ddosh0Bze5zy-nQ6gAJFpBfL13muCXrTyKYTps61bmnUxpp3bJnt_2N2MXGfuxBENO0Rbc8DhVPd-lNa4H3XjMwIBdxDAwW32z3pfVr8pHyWxeFtK4SCbvX8B0C6n8ZHigJsvdiCNmoj7_LO_QUzIXmXLFvEXtAqzD_hCr0pJxRIr0BrBjYwL23PkxOYzBR-URcd4Ilji6410Eh9NXycyFzKOcqZ7rjG_PnRyUX1EBZH_PN4RExjJuXYgiqhtU-tDjQFzXLhvwAd5s3ThP9lax27A6MUpjLSKkNy-dG5tlaA0QvECfDzA-5eQjcL_OfvbHlKHQH9zPh-U9Q8gsf3iXmbJrypkalUiTCqnzJu5TgZORSg6zmxNyOCz53YxBHEEaF8yROPwxWDylZfC4fxCRTdoAyFgmFLfMbiepV7AZ24KLj4jfMbGfKpkbPq0xirnSAS-3vbOfkgko5X420AttP8Z1ZBbFSD20Ath_TA9PSHiRCak4AXvOoCZg0t-WuMwzkd_B2V_JZZSTb1yBWrKTL1QzUamqlufjdWuz7M-O2Wkb2cyDSESVNuQyJgDkYb0AOWo0BaN3wbOeT_D4cSrjQoo01xQQCZHQ9SVR4QzUQNAiQcSriqEiptHYhbi6R5_GfGAeMHmlJa4atO2hense0Qk4vDc2fc-sbnQ1jPiE",
    "e": "AQAB",
    "key_ops": [
      "verify",
    ],
    "ext": true,
  };

  const keyObject = createPublicKey({ key, format: "jwk" });
  const expectedPem = `-----BEGIN PUBLIC KEY-----
MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEA5Ddosh0Bze5zy+nQ6gAJ
FpBfL13muCXrTyKYTps61bmnUxpp3bJnt/2N2MXGfuxBENO0Rbc8DhVPd+lNa4H3
XjMwIBdxDAwW32z3pfVr8pHyWxeFtK4SCbvX8B0C6n8ZHigJsvdiCNmoj7/LO/QU
zIXmXLFvEXtAqzD/hCr0pJxRIr0BrBjYwL23PkxOYzBR+URcd4Ilji6410Eh9NXy
cyFzKOcqZ7rjG/PnRyUX1EBZH/PN4RExjJuXYgiqhtU+tDjQFzXLhvwAd5s3ThP9
lax27A6MUpjLSKkNy+dG5tlaA0QvECfDzA+5eQjcL/OfvbHlKHQH9zPh+U9Q8gsf
3iXmbJrypkalUiTCqnzJu5TgZORSg6zmxNyOCz53YxBHEEaF8yROPwxWDylZfC4f
xCRTdoAyFgmFLfMbiepV7AZ24KLj4jfMbGfKpkbPq0xirnSAS+3vbOfkgko5X420
AttP8Z1ZBbFSD20Ath/TA9PSHiRCak4AXvOoCZg0t+WuMwzkd/B2V/JZZSTb1yBW
rKTL1QzUamqlufjdWuz7M+O2Wkb2cyDSESVNuQyJgDkYb0AOWo0BaN3wbOeT/D4c
SrjQoo01xQQCZHQ9SVR4QzUQNAiQcSriqEiptHYhbi6R5/GfGAeMHmlJa4atO2he
nse0Qk4vDc2fc+sbnQ1jPiECAwEAAQ==
-----END PUBLIC KEY-----
`;

  const pem = keyObject.export({ format: "pem", type: "spki" });
  assertEquals(pem, expectedPem);
});

Deno.test("Ed25519 import jwk public key #1", function () {
  const key = {
    "kty": "OKP",
    "crv": "Ed25519",
    "d": "nWGxne_9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A",
    "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo",
  };
  const keyObject = createPublicKey({ key, format: "jwk" });

  assertEquals(keyObject.type, "public");
  const spkiActual = keyObject.export({ type: "spki", format: "pem" });

  const spkiExpected = `-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEA11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=
-----END PUBLIC KEY-----
`;

  assertEquals(spkiActual, spkiExpected);
});

Deno.test("Ed25519 import jwk public key #2", function () {
  const key = {
    "kty": "OKP",
    "crv": "Ed25519",
    "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo",
  };

  const keyObject = createPublicKey({ key, format: "jwk" });
  assertEquals(keyObject.type, "public");

  const spki = keyObject.export({ type: "spki", format: "pem" });
  const spkiExpected = `-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEA11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=
-----END PUBLIC KEY-----
`;
  assertEquals(spki, spkiExpected);
});

Deno.test("Ed25519 import jwk private key", function () {
  const key = {
    "kty": "OKP",
    "crv": "Ed25519",
    "d": "nWGxne_9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A",
    "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo",
  };

  const keyObject = createPrivateKey({ key, format: "jwk" });
  assertEquals(keyObject.type, "private");

  const pkcs8Actual = keyObject.export({ type: "pkcs8", format: "pem" });
  const pkcs8Expected = `-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIJ1hsZ3v/VpguoRK9JLsLMREScVpezJpGXA7rAMcrn9g
-----END PRIVATE KEY-----
`;

  assertEquals(pkcs8Actual, pkcs8Expected);
});

Deno.test("RSA export public JWK", function () {
  const importKey = "-----BEGIN PUBLIC KEY-----\n" +
    "MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAqF66soiDvuqUB7ufWtuV\n" +
    "5a1nZIw90m9qHEl2MeNt66HeEjG2GeHDfF5a4uplutnAh3dwpFweHqGIyB16POTI\n" +
    "YysJ/rMPKoWZFQ1LEcr23rSgmL49YpifDetl5V/UR+zEygL3UzzZmbdjuyZz+Sjt\n" +
    "FY+SAoZ9XPCqIaNha9uVFcurW44MvAkhzQR/yy5NWPaJ/yv4oI/exvuZnUwwBHvH\n" +
    "gwVchfr7Jh5LRmYTPeyuI1lUOovVzE+0Ty/2tFfrm2hpedqYXvEuVu+yJzfuNoLf\n" +
    "TGfz15J76eoRdFTCTdaG/MQnrzxZnIlmIpdpTPl0xVOwjKRpeYK06GS7EAa7cS9D\n" +
    "dnsHkF/Mr9Yys5jw/49fXqh9BH3Iy0p5YmeQIMep04CUDFj7MZ+3SK8b0mA4SscH\n" +
    "dIraZZynLZ1crM0ECAJBldM4TKqIDACYGU7XyRV+419cPJvYybHys5m7thS3QI7E\n" +
    "LTpMV+WoYtZ5xeBCm7z5i3iPY6eSh2JtTu6oa3ALwwnXPAaZqDIFer8SoQNyVb0v\n" +
    "EU8bVDeGXm1ha5gcC5KxqqnadO/WDD6Jke79Ji04sBEKTTodSOARyTGpGFEcC3Nn\n" +
    "xSSScGCxMrGJuTDtnz+Eh6l6ysT+Nei9ZRMxNu8sZKAR43XkVXxF/OdSCbftFOAs\n" +
    "wyPJtyhQALGPcK5cWPQS2sUCAwEAAQ==\n" +
    "-----END PUBLIC KEY-----\n";
  const publicKey = createPublicKey(importKey);

  const jwk = publicKey.export({ format: "jwk" });
  assertEquals(jwk, {
    kty: "RSA",
    n: "qF66soiDvuqUB7ufWtuV5a1nZIw90m9qHEl2MeNt66HeEjG2GeHDfF5a4uplutnAh3dwpFweHqGIyB16POTIYysJ_rMPKoWZFQ1LEcr23rSgmL49YpifDetl5V_UR-zEygL3UzzZmbdjuyZz-SjtFY-SAoZ9XPCqIaNha9uVFcurW44MvAkhzQR_yy5NWPaJ_yv4oI_exvuZnUwwBHvHgwVchfr7Jh5LRmYTPeyuI1lUOovVzE-0Ty_2tFfrm2hpedqYXvEuVu-yJzfuNoLfTGfz15J76eoRdFTCTdaG_MQnrzxZnIlmIpdpTPl0xVOwjKRpeYK06GS7EAa7cS9DdnsHkF_Mr9Yys5jw_49fXqh9BH3Iy0p5YmeQIMep04CUDFj7MZ-3SK8b0mA4SscHdIraZZynLZ1crM0ECAJBldM4TKqIDACYGU7XyRV-419cPJvYybHys5m7thS3QI7ELTpMV-WoYtZ5xeBCm7z5i3iPY6eSh2JtTu6oa3ALwwnXPAaZqDIFer8SoQNyVb0vEU8bVDeGXm1ha5gcC5KxqqnadO_WDD6Jke79Ji04sBEKTTodSOARyTGpGFEcC3NnxSSScGCxMrGJuTDtnz-Eh6l6ysT-Nei9ZRMxNu8sZKAR43XkVXxF_OdSCbftFOAswyPJtyhQALGPcK5cWPQS2sU",
    e: "AQAB",
  });
});

Deno.test("EC export public jwk", function () {
  const key = "-----BEGIN PUBLIC KEY-----\n" +
    "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEVEEIrFEZ+40Pk90LtKBQ3r7FGAPl\n" +
    "v4bvX9grC8bNiNiVAcyEKs+QZKQj/0/CUPJV10AmavrUoPk/7Wy0sejopQ==\n" +
    "-----END PUBLIC KEY-----\n";
  const publicKey = createPublicKey(key);

  const jwk = publicKey.export({ format: "jwk" });
  assertEquals(jwk, {
    kty: "EC",
    x: "VEEIrFEZ-40Pk90LtKBQ3r7FGAPlv4bvX9grC8bNiNg",
    y: "lQHMhCrPkGSkI_9PwlDyVddAJmr61KD5P-1stLHo6KU",
    crv: "P-256",
  });
});

Deno.test("Ed25519 export public jwk", function () {
  const key = "-----BEGIN PUBLIC KEY-----\n" +
    "MCowBQYDK2VwAyEAKCVFOD6Le61XM7HbN/MB/N06mX5bti2p50qjLvT1mzE=\n" +
    "-----END PUBLIC KEY-----\n";
  const publicKey = createPublicKey(key);

  const jwk = publicKey.export({ format: "jwk" });
  assertEquals(jwk, {
    crv: "Ed25519",
    x: "KCVFOD6Le61XM7HbN_MB_N06mX5bti2p50qjLvT1mzE",
    kty: "OKP",
  });
});

Deno.test("EC import jwk public key", function () {
  const publicKey = createPublicKey({
    key: {
      kty: "EC",
      x: "_GGuz19zab5J70zyiUK6sAM5mHqUbsY8H6U2TnVlt-k",
      y: "TcZG5efXZDIhNGDp6XuujoJqOEJU2D2ckjG9nOnSPIQ",
      crv: "P-256",
    },
    format: "jwk",
  });

  const publicSpki = publicKey.export({ type: "spki", format: "pem" });
  const spkiExpected = `-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE/GGuz19zab5J70zyiUK6sAM5mHqU
bsY8H6U2TnVlt+lNxkbl59dkMiE0YOnpe66Ogmo4QlTYPZySMb2c6dI8hA==
-----END PUBLIC KEY-----
`;

  assertEquals(publicSpki, spkiExpected);
});

Deno.test("EC import jwk private key", function () {
  const privateKey = createPrivateKey({
    key: {
      kty: "EC",
      x: "_GGuz19zab5J70zyiUK6sAM5mHqUbsY8H6U2TnVlt-k",
      y: "TcZG5efXZDIhNGDp6XuujoJqOEJU2D2ckjG9nOnSPIQ",
      crv: "P-256",
      d: "Wobjne0GqlB_1NynKu19rsw7zBHa94tKcWIxwIb88m8",
    },
    format: "jwk",
  });

  const privatePkcs8 = privateKey.export({ type: "pkcs8", format: "pem" });

  const pkcs8Expected = `-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWobjne0GqlB/1Nyn
Ku19rsw7zBHa94tKcWIxwIb88m+hRANCAAT8Ya7PX3NpvknvTPKJQrqwAzmYepRu
xjwfpTZOdWW36U3GRuXn12QyITRg6el7ro6CajhCVNg9nJIxvZzp0jyE
-----END PRIVATE KEY-----
`;

  assertEquals(privatePkcs8, pkcs8Expected);
});

Deno.test("createPublicKey x509", function () {
  const certificate = `-----BEGIN CERTIFICATE-----
MIIC8zCCAdugAwIBAgIBATANBgkqhkiG9w0BAQsFADAbMRkwFwYDVQQDExB0ZXN0
LWNlcnRpZmljYXRlMB4XDTI0MDkxNzA5MTczNVoXDTI3MDkxNzA5MTczNVowGzEZ
MBcGA1UEAxMQdGVzdC1jZXJ0aWZpY2F0ZTCCASIwDQYJKoZIhvcNAQEBBQADggEP
ADCCAQoCggEBAMOzTIrwvbUbPIrxCr5DO1XMd3tH37pID0no4cOUq1hxNEnB4l1j
2201atvmXwzWI3xtPzfwOYUtE/DGagFh805/nod5yXwR6liGd9RjkABxPi0UF7jl
lWHfBLyILUHVR9hEOl65vUpKx5ORNgbO9L7WsL/FKH9pvCbWjdI29+pQnZ4gOoWZ
YC6auoKfG7TcbaFb9AubolcIlofC2MHP+cWjPA+iX6ezUqqN1Ug5xGiF/sC79M0o
5d6E83zdXxyyFwydUWUv3EKgmVTLln/2hYQFKCRhy72n6L7y9JNcieOauQK0efJB
+2HwaWeAr2xkhSnWVCRl4nEgiu/E0nL/zNUCAwEAAaNCMEAwDgYDVR0PAQH/BAQD
AgGGMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFIAeXho137l8V6daKI33IvRb
N6CyMA0GCSqGSIb3DQEBCwUAA4IBAQAQU1Sast6VsD4uTJiSz/lSEkLZ6wC/6v+R
az0YSnbNmQ5YczBLdTLs07hBC1tDvv0vfopRXvNxP7AxkopX5O7Lc15zf5JdTZnY
/tJwO62jZiaLsfAo2JzrZ31h2cFFFRTYPTx+8E4djgdmwKtaECeQFXqdpOHOJCGv
NfwVlZ7Z/cd8fI8oiNtvJDAhPa/UZXAhFV74hT0DiuMwPiJvsG83rutvAYpZ8lPu
yG6QSsxPnxzEHIKR+vgxUHKwTdv0sWt3XBmpIY5CGXFR2eIQP1jv0ohtcnLMJe8N
z6TExWlQMjt66nV7R8cRAkzmABrG+NW3e8Zpac7Lkuv+zu0S+K7c
-----END CERTIFICATE-----`;

  const publicKey = createPublicKey(certificate);
  assertEquals(publicKey.type, "public");
  assertEquals(publicKey.asymmetricKeyType, "rsa");
});
