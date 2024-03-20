// deno-lint-ignore-file no-explicit-any

// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  Certificate,
  createECDH,
  createHmac,
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  generateKeyPair,
  generateKeyPairSync,
  KeyObject,
  randomBytes,
} from "node:crypto";
import { promisify } from "node:util";
import { Buffer } from "node:buffer";
import { assertEquals, assertThrows } from "@std/assert/mod.ts";

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

Deno.test("spkac Certificate", function () {
  const rsaSpkac =
    `MIICUzCCATswggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC33FiIiiexwLe/P8DZx5HsqFlmUO7/lvJ7necJVNwqdZ3ax5jpQB0p6uxfqeOvzcN3k5V7UFb/Am+nkSNZMAZhsWzCU2Z4Pjh50QYz3f0Hour7/yIGStOLyYY3hgLK2K8TbhgjQPhdkw9+QtKlpvbL8fLgONAoGrVOFnRQGcr70iFffsm79mgZhKVMgYiHPJqJgGHvCtkGg9zMgS7p63+Q3ZWedtFS2RhMX3uCBy/mH6EOlRCNBbRmA4xxNzyf5GQaki3T+Iz9tOMjdPP+CwV2LqEdylmBuik8vrfTb3qIHLKKBAI8lXN26wWtA3kN4L7NP+cbKlCRlqctvhmylLH1AgMBAAEWE3RoaXMtaXMtYS1jaGFsbGVuZ2UwDQYJKoZIhvcNAQEEBQADggEBAIozmeW1kfDfAVwRQKileZGLRGCD7AjdHLYEe16xTBPve8Af1bDOyuWsAm4qQLYA4FAFROiKeGqxCtIErEvm87/09tCfF1My/1Uj+INjAk39DK9J9alLlTsrwSgd1lb3YlXY7TyitCmh7iXLo4pVhA2chNA3njiMq3CUpSvGbpzrESL2dv97lv590gUD988wkTDVyYsf0T8+X0Kww3AgPWGji+2f2i5/jTfD/s1lK1nqi7ZxFm0pGZoy1MJ51SCEy7Y82ajroI+5786nC02mo9ak7samca4YDZOoxN4d3tax4B/HDF5dqJSm1/31xYLDTfujCM5FkSjRc4m6hnriEkc=`;

  const challenge = Certificate.exportChallenge(rsaSpkac);
  assertEquals(challenge.toString(), "this-is-a-challenge");

  const publicKey = Certificate.exportPublicKey(rsaSpkac);
  const expected = "-----BEGIN PUBLIC KEY-----\n" +
    "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAt9xYiIonscC3vz/A2ceR\n" +
    "7KhZZlDu/5bye53nCVTcKnWd2seY6UAdKersX6njr83Dd5OVe1BW/wJvp5EjWTAG\n" +
    "YbFswlNmeD44edEGM939B6Lq+/8iBkrTi8mGN4YCytivE24YI0D4XZMPfkLSpab2\n" +
    "y/Hy4DjQKBq1ThZ0UBnK+9IhX37Ju/ZoGYSlTIGIhzyaiYBh7wrZBoPczIEu6et/\n" +
    "kN2VnnbRUtkYTF97ggcv5h+hDpUQjQW0ZgOMcTc8n+RkGpIt0/iM/bTjI3Tz/gsF\n" +
    "di6hHcpZgbopPL630296iByyigQCPJVzdusFrQN5DeC+zT/nGypQkZanLb4ZspSx\n" +
    "9QIDAQAB\n" +
    "-----END PUBLIC KEY-----\n";
  assertEquals(publicKey.toString(), expected);
});
