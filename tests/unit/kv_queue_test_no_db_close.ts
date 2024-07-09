// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertNotEquals } from "./test_util.ts";

Deno.test({
  sanitizeOps: false,
  sanitizeResources: false,
}, async function queueTestNoDbClose() {
  const db: Deno.Kv = await Deno.openKv(":memory:");
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;
  db.listenQueue((msg) => {
    dequeuedMessage = msg;
    resolve();
  });
  const res = await db.enqueue("test");
  assert(res.ok);
  assertNotEquals(res.versionstamp, null);
  await promise;
  assertEquals(dequeuedMessage, "test");
});
