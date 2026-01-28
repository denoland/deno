// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, fail } from "@std/assert";
import { closeSync, fdatasync, fdatasyncSync, openSync, writeSync } from "node:fs";

Deno.test({
  name:
    "ASYNC: flush any pending data operations of the given file stream to disk",
  async fn() {
    const filePath = await Deno.makeTempFile();
    const fd = openSync(filePath, "rs+");
    const data = new Uint8Array(64);
    writeSync(fd, data);

    await new Promise<void>((resolve, reject) => {
      fdatasync(fd, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        async () => {
          assertEquals(await Deno.readFile(filePath), data);
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
  name:
    "SYNC: flush any pending data operations of the given file stream to disk.",
  fn() {
    const filePath = Deno.makeTempFileSync();
    const fd = openSync(filePath, "rs+");
    const data = new Uint8Array(64);
    writeSync(fd, data);

    try {
      fdatasyncSync(fd);
      assertEquals(Deno.readFileSync(filePath), data);
    } finally {
      closeSync(fd);
      Deno.removeSync(filePath);
    }
  },
});
