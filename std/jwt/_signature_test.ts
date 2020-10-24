import { assertEquals } from "../testing/asserts.ts";
import { create, decode } from "./mod.ts";

import {
  convertHexToBase64url,
  create as createSignature,
  verify as verifySignature,
} from "./_signature.ts";

const algorithm = "HS256";
const key = "m$y-key";

Deno.test("[jwt] create signature", async function () {
  // https://www.freeformatter.com/hmac-generator.html
  const computedHmacInHex =
    "2b9e6619fa7f2c8d8b3565c88365376b75b1b0e5d87e41218066fd1986f2c056";
  assertEquals(
    await createSignature(algorithm, key, "thisTextWillBeEncrypted"),
    convertHexToBase64url(computedHmacInHex),
  );

  const anotherVerifiedSignatureInBase64Url =
    "p2KneqJhji8T0PDlVxcG4DROyzTgWXbDhz_mcTVojXo";
  assertEquals(
    await createSignature(
      algorithm,
      key,
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ",
    ),
    anotherVerifiedSignatureInBase64Url,
  );
});

Deno.test("[jwt] verify signature", async function () {
  const jwt = await create({}, key);
  const { header, signature } = decode(jwt);

  const validSignature = await verifySignature({
    signature,
    key,
    algorithm: header.alg,
    signingInput: jwt.slice(0, jwt.lastIndexOf(".")),
  });

  assertEquals(validSignature, true);
});
