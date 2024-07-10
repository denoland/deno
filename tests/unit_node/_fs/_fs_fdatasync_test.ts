// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "@std/assert/mod.ts";
import { fdatasync, fdatasyncSync } from "node:fs";

Deno.test({
  name:
    "ASYNC: flush any pending data operations of the given file stream to disk",
  async fn() {
    const filePath = await Deno.makeTempFile();
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    await file.write(data);

    await new Promise<void>((resolve, reject) => {
      fdatasync(file.rid, (err: Error | null) => {
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
        await Deno.remove(filePath);
      });
  },
});

Deno.test({
  name:
    "SYNC: flush any pending data operations of the given file stream to disk.",
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    file.writeSync(data);

    try {
      fdatasyncSync(file.rid);
      assertEquals(Deno.readFileSync(filePath), data);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});
