import { assert, assertEquals, assertThrows } from "../testing/asserts.ts"

import {
  verify as verifyAlgorithm,
} from "./algorithm.ts"

Deno.test("[jwt] verify algorithm", async function (): Promise<void> {
  assertEquals(verifyAlgorithm(["HS512"], "HS512"), true)
  assertEquals(verifyAlgorithm(["HS256", "HS512"], "HS512"), true)
  assertEquals(verifyAlgorithm(["HS512"], "HS256"), false)
})
