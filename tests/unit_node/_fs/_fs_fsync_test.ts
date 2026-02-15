// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, fail } from "@std/assert";
import { closeSync, fsync, fsyncSync, openSync } from "node:fs";

Deno.test({
  name: "ASYNC: flush any pending data of the given file stream to disk",
  async fn() {
    const filePath = await Deno.makeTempFile();
    const fd = openSync(filePath, "rs+");
    const size = 64;
    Deno.truncateSync(filePath, size);

    await new Promise<void>((resolve, reject) => {
      fsync(fd, (err: Error | null) => {
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
        closeSync(fd);
        await Deno.remove(filePath);
      });
  },
});

Deno.test({
  name: "SYNC: flush any pending data the given file stream to disk",
  fn() {
    const filePath = Deno.makeTempFileSync();
    const fd = openSync(filePath, "rs+");
    const size = 64;
    Deno.truncateSync(filePath, size);

    try {
      fsyncSync(fd);
      assertEquals(Deno.statSync(filePath).size, size);
    } finally {
      closeSync(fd);
      Deno.removeSync(filePath);
    }
  },
});
