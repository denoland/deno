import { assertEquals, assertThrows } from "../testing/asserts.ts"
import { create, parse } from "./mod.ts"

import {
  create as createSignature,
  verify as verifySignature,
} from "./signature.ts"
import { convertHexToBase64url } from "./_util.ts"

const algorithm = "HS256"
const key = "m$y-key"

Deno.test("[jwt] create signature", async function (): Promise<void> {
  // https://www.freeformatter.com/hmac-generator.html
  const computedHmacInHex = "2b9e6619fa7f2c8d8b3565c88365376b75b1b0e5d87e41218066fd1986f2c056"
  assertEquals(
    await createSignature(algorithm, key, "thisTextWillBeEncrypted"),
    convertHexToBase64url(computedHmacInHex),
  )

  const anotherVerifiedSignatureInBase64Url = "p2KneqJhji8T0PDlVxcG4DROyzTgWXbDhz_mcTVojXo"
  assertEquals(
    await createSignature(
      algorithm,
      key,
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ",
    ),
    anotherVerifiedSignatureInBase64Url,
  )
})

Deno.test("[jwt] verify signature", async function (): Promise<void> {
  const jwt = await create({
    payload: {},
    key
  })
  const { header, signature } = await parse(jwt);

  const validSignature = await verifySignature({
    signature,
    key,
    alg: header.alg,
    signingInput: jwt.slice(0, jwt.lastIndexOf(".")),
  })
  
  assertEquals(validSignature, true)
})
