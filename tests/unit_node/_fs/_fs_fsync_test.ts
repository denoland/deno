// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "@std/assert/mod.ts";
import { fsync, fsyncSync } from "node:fs";

Deno.test({
  name: "ASYNC: flush any pending data of the given file stream to disk",
  async fn() {
    const filePath = await Deno.makeTempFile();
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    await file.truncate(size);

    await new Promise<void>((resolve, reject) => {
      fsync(file.rid, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        async () => {
          assertEquals((await Deno.stat(filePath)).size, size);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(async () => {
        await Deno.remove(filePath);
      });
  },
});

Deno.test({
  name: "SYNC: flush any pending data the given file stream to disk",
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    file.truncateSync(size);

    try {
      fsyncSync(file.rid);
      assertEquals(Deno.statSync(filePath).size, size);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});
