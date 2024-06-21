// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { randomFillSync, randomUUID } from "node:crypto";
import { assert, assertEquals } from "../../unit/test_util.ts";

Deno.test("[node/crypto.getRandomUUID] works the same way as Web Crypto API", () => {
  assertEquals(randomUUID().length, crypto.randomUUID().length);
  assertEquals(typeof randomUUID(), typeof crypto.randomUUID());
});

Deno.test("[node/crypto.randomFillSync] supported arguments", () => {
  const buf = new Uint8Array(10);

  assert(randomFillSync(buf));
  assert(randomFillSync(buf, 0));
  // @ts-ignore: arraybuffer arguments are valid.
  assert(randomFillSync(buf.buffer));
  assert(randomFillSync(new DataView(buf.buffer)));
});
