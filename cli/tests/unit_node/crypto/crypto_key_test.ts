// deno-lint-ignore-file no-explicit-any

// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  createPrivateKey,
  createSecretKey,
  generateKeyPair,
  generateKeyPairSync,
  KeyObject,
  randomBytes,
} from "node:crypto";
import { promisify } from "node:util";
import { Buffer } from "node:buffer";
import {
  assertEquals,
  assertThrows,
} from "../../../../test_util/std/assert/mod.ts";
import { createHmac } from "node:crypto";

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

for (const namedCurve of ["P-384", "P-256"]) {
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
