// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test({}, async function queueTestDbClose() {
  const db: Deno.Kv = await Deno.openKv(":memory:");
  db.close();
  try {
    await db.listenQueue(() => {});
  } catch (e) {
    assertEquals(e.message, "already closed");
  }
});
