// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "../../testing/asserts.ts";
import { fdatasync, fdatasyncSync } from "./_fs_fdatasync.ts";

Deno.test({
  name:
    "ASYNC: flush any pending data operations of the given file stream to disk",
  async fn() {
    const file: string = await Deno.makeTempFile();
    const { rid } = await Deno.open(file, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    await Deno.write(rid, data);

    await new Promise<void>((resolve, reject) => {
      fdatasync(rid, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        async () => {
          assertEquals(await Deno.readFile(file), data);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(async () => {
        Deno.close(rid);
        await Deno.remove(file);
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
