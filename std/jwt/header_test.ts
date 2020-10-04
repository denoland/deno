import { assertEquals, assertThrows, assertThrowsAsync } from "../testing/asserts.ts"

import {
  verify,
  create,
} from "./mod.ts"
import { Handlers, verifyHeaderCrit } from "./header.ts"
import { setExpiration, Header } from "./mod.ts"

const key = "m$y-key"

Deno.test("[jwt] verifyHeaderCrit", async function (): Promise<void> {
  const header: Header = {
    alg: "HS256",
    crit: ["dummy", "asyncDummy"],
    dummy: 100,
    asyncDummy: 200,
  }
  const critHandlers: Handlers = {
    dummy(value) {
      return value
    },
    async asyncDummy(value) {
      return value
    },
  }
  await verifyHeaderCrit(header, critHandlers)
})

Deno.test("[jwt] critHandlers", async function (): Promise<void> {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(1),
  }
  const header: Header = {
    alg: "HS256",
    crit: ["dummy"],
    dummy: 100,
  }
  const critHandlers = {
    dummy(value: any) {
      return value * 2
    },
  }

  const jwt = await create({ header, payload, key })
  assertEquals(await verify({ jwt, key, critHandlers, algorithm: ["HS256", "HS512"], }), payload)
  assertThrowsAsync(async () => await verify({ jwt, key, algorithm: "HS256" }), Error)
})
