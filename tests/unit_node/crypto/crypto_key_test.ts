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
import { assert, assertEquals, assertThrows } from "@std/assert/mod.ts";

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
