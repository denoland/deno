// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "@std/assert";
import {
  createSign,
  createVerify,
  generateKeyPairSync,
  sign,
  verify,
} from "node:crypto";
import { Buffer } from "node:buffer";
import fixtures from "../testdata/crypto_digest_fixtures.json" with {
  type: "json",
};

const rsaPrivatePem = Buffer.from(
  await Deno.readFile(
    new URL("../testdata/rsa_private.pem", import.meta.url),
  ),
);
const rsaPrivatePkcs1Pem = Buffer.from(
  await Deno.readFile(
    new URL("../testdata/rsa_private_pkcs1.pem", import.meta.url),
  ),
);
const rsaPublicPem = Buffer.from(
  await Deno.readFile(
    new URL("../testdata/rsa_public.pem", import.meta.url),
  ),
);

const table = [
  {
    algorithms: ["sha224", "RSA-SHA224"],
    signature:
      "7ad162b288bd7f4ba9b8a31295ad4136d143a5fd11eb99a72379dc9b53e3e8b5c1b7c9dd8a3864a1f626d921e550c48056982bd8fe7e75333885311b5515de1ecbbfcc6a1dd930f422dff87bfceb7eb38882ac6b4fd9dea9efd462776775976e81b1d677f8db41f5ac8686abfa9838069125be939c59e404aa50550872d84befb8b5f6ce2dd051c62a8ba268f876b6f17a27af43b79938222e4ab8b90c4f5540d0f8b02508ef3e68279d685746956b924f00c92438b7981a3cfcb1e2a97305402d381ea62aeaa803f8707961bc3e10a258352e210772e9846ca4024e3dc0a956a50d6db1c03d2943826cc98c6f36d7bafacf1c94b6c438c7664c300a3be172b1",
  },
  {
    algorithms: ["sha256", "RSA-SHA256"],
    signature:
      "080313284d7398e1e0e27f6e44f198ceecedddc801e81af63a867d9245ad744e29018099c9ac3c27061c33cabfe27af1db38f44bac09cdcd2c4ab3b00a2a3020f68368f2239db5f911a2dbb7ea2dee322ca7d26d0c88d197482ca4aa1c29ac87b9e6c20075dc974ae71d2d76d2a5b2a15bd541033519465c3aea815cc73b0f1c3ffeedcfb93d6788416623789f86786870d23e86b982ab0df157d7a596097bd3cca3e752f3f47eff4b83754296868b52bc8ff741492dc8a401fe6dc035569e45d1fa1a71c8988d3aadce68fb1bf5c3e756c586af20c8e75c037436ff4c8389e6ce9d943ef7e2566977b84577272181fcec403077cc29e7db1166fff900b36a1d",
  },
  {
    algorithms: ["sha384", "RSA-SHA384"],
    signature:
      "2f77a5b7ac0168efd652c30ecb082075f3de30629e9c1f51b7e7e671f24b5c3a2606bb72159a217438220fc7aaba887d4b817e3f43fe0cc8f840747368df8cd65ec760c21a3f9296d01caedc80a335030e31d31ac451277fc4bcc1679c168b2c3185dfee21286514113c080af5238a61a677b03777344f476f25053108588aa6bdc02a6138c6b59a20de4d11e3d668482f17e748e75747f83c0512206283acfc64ed0ad963dddc9ec24589cfd459ee806b8e0e67b93cea16651e967762a5deef890f438ffb9db39247469289db06e2ed7fe262aa1df4ab9607e5b5219a17ddc9694283a61bf8643f58fd702f2c5d3b2d53dc7f36bb5e96461174d376950d6d19",
  },
  {
    algorithms: ["sha512", "RSA-SHA512"],
    signature:
      "072e20a433f255ab2f7e5e9ce69255d5c6d7c15a36af75c8389b9672c41abc6a9532fbd057d9d64270bb2483d3c9923f8f419fba4b59b838dcda82a1322009d245c06e2802a74febaea9cebc0b7f46f8761331c5f52ffb650245b5aefefcc604f209b44f6560fe45370cb239d236622e5f72fbb45377f08a0c733e16a8f15830897679ad4349d2e2e5e50a99796820302f4f47881ed444aede56a6d3330b71acaefc4218ae2e4a3bdfbb0c9432ffc5e5bac8c168278b2205d68a5d6905ccbb91282d519c11eccca52d42c86787de492b2a89679dce98cd14c37b0c183af8427e7a1ec86b1ed3f9b5bebf83f1ef81eb18748e69c716a0f263a8598fe627158647",
  },
];

const data = Buffer.from("some data to sign");

Deno.test({
  name:
    "crypto.Sign|sign - RSA PEM with SHA224, SHA256, SHA384, SHA512 digests",
  fn() {
    for (const testCase of table) {
      for (const algorithm of testCase.algorithms) {
        assertEquals(
          createSign(algorithm)
            .update(data)
            .sign(rsaPrivatePem, "hex"),
          testCase.signature,
        );
        assertEquals(
          sign(algorithm, data, rsaPrivatePem),
          Buffer.from(testCase.signature, "hex"),
        );
      }
    }
  },
});

Deno.test({
  name:
    "crypto.Verify|verify - RSA PEM with SHA224, SHA256, SHA384, SHA512 digests",
  fn() {
    for (const testCase of table) {
      for (const algorithm of testCase.algorithms) {
        assert(
          createVerify(algorithm).update(data).verify(
            rsaPublicPem,
            testCase.signature,
            "hex",
          ),
        );
        assert(
          verify(
            algorithm,
            data,
            rsaPublicPem,
            Buffer.from(testCase.signature, "hex"),
          ),
        );
      }
    }
  },
});

Deno.test({
  name: "crypto.createPrivateKey|sign - RSA PEM",
  fn() {
    for (const testCase of table) {
      for (const algorithm of testCase.algorithms) {
        assertEquals(
          createSign(algorithm).update(data).sign(rsaPrivatePem, "hex"),
          testCase.signature,
        );
        assertEquals(
          sign(algorithm, data, rsaPrivatePem),
          Buffer.from(testCase.signature, "hex"),
        );
      }
    }
  },
});

Deno.test({
  name: "crypto.createPrivateKey|sign - RSA PKCS1 PEM",
  fn() {
    for (const testCase of table) {
      for (const algorithm of testCase.algorithms) {
        assertEquals(
          createSign(algorithm).update(data).sign(rsaPrivatePkcs1Pem, "hex"),
          testCase.signature,
        );
        assertEquals(
          sign(algorithm, data, rsaPrivatePkcs1Pem),
          Buffer.from(testCase.signature, "hex"),
        );
      }
    }
  },
});

Deno.test({
  name: "crypto.createSign|sign - EC PRIVATE KEY",
  fn() {
    const pem = `-----BEGIN EC PRIVATE KEY-----
MDECAQEEIIThPSZ00CNW1UD5Ju9mhplv6SSs3T5objYjlx11gHW9oAoGCCqGSM49
AwEH
-----END EC PRIVATE KEY-----`;
    createSign("SHA256").update("test").sign(pem, "base64");
  },
});

Deno.test("crypto.createSign|sign - compare with node", async (t) => {
  const DATA = "Hello, world!";
  const privateKey = Deno.readTextFileSync(
    new URL(import.meta.resolve("../testdata/rsa_private.pem")),
  );
  for (const { digest, signature } of fixtures) {
    await t.step({
      name: digest,
      // TODO(lucacasonato): our md4 implementation does not have an OID, so it can't sign/verify
      ignore: digest.toLowerCase().includes("md4"),
      fn: () => {
        let actual: string | null;
        try {
          const s = createSign(digest);
          s.update(DATA);
          actual = s.sign(privateKey).toString("hex");
        } catch {
          actual = null;
        }
        assertEquals(actual, signature);
      },
    });
  }
});

Deno.test("crypto.createVerify|verify - compare with node", async (t) => {
  const DATA = "Hello, world!";
  const publicKey = Deno.readTextFileSync(
    new URL(import.meta.resolve("../testdata/rsa_public.pem")),
  );
  for (const { digest, signature } of fixtures) {
    await t.step({
      name: digest,
      // TODO(lucacasonato): our md4 implementation does not have an OID, so it can't sign/verify
      ignore: signature === null || digest.toLowerCase().includes("md4"),
      fn: () => {
        const s = createVerify(digest);
        s.update(DATA);
        s.verify(publicKey, signature!);
      },
    });
  }
});

Deno.test("crypto sign|verify dsaEncoding", () => {
  const { privateKey, publicKey } = generateKeyPairSync("ec", {
    namedCurve: "P-256",
  });

  const sign = createSign("SHA256");
  sign.write("some data to sign");
  sign.end();

  // @ts-ignore FIXME: types dont allow this
  privateKey.dsaEncoding = "ieee-p1363";
  const signature = sign.sign(privateKey, "hex");

  const verify = createVerify("SHA256");
  verify.write("some data to sign");
  verify.end();

  // @ts-ignore FIXME: types dont allow this
  publicKey.dsaEncoding = "ieee-p1363";
  assert(verify.verify(publicKey, signature, "hex"));
});
