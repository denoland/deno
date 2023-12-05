// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertRejects } from "./test_util.ts";

Deno.test({}, async function queueTestDbClose() {
  const db: Deno.Kv = await Deno.openKv(":memory:");
  db.close();
  await assertRejects(
    async () => {
      await db.listenQueue(() => {});
    },
    Error,
    "already closed",
  );
});
