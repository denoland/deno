// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "../../../../test_util/std/assert/mod.ts";
import { fdatasync, fdatasyncSync } from "node:fs";

Deno.test({
  name:
    "ASYNC: flush any pending data operations of the given file stream to disk",
  async fn() {
    const filePath = await Deno.makeTempFile();
    const file = await Deno.open(filePath, {
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
        Deno.close(file.rid);
        await Deno.remove(filePath);
      });
  },
});

Deno.test({
  name:
    "SYNC: flush any pending data operations of the given file stream to disk.",
  fn() {
    const file: string = Deno.makeTempFileSync();
    const { rid } = Deno.openSync(file, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    Deno.writeSync(rid, data);

    try {
      fdatasyncSync(rid);
      assertEquals(Deno.readFileSync(file), data);
    } finally {
      Deno.close(rid);
      Deno.removeSync(file);
    }
  },
});
