// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertFalse } from "./test_util.ts";

Deno.test({}, async function queueTestDbClose() {
  const db: Deno.Kv = await Deno.openKv(":memory:");
  db.close();
  try {
    await db.listenQueue(() => {});
    assertFalse(false);
  } catch (e) {
    assertEquals((e as Error).message, "Queue already closed");
  }
});
