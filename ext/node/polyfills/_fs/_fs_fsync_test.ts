// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "../../testing/asserts.ts";
import { fsync, fsyncSync } from "./_fs_fsync.ts";

Deno.test({
  name: "ASYNC: flush any pending data of the given file stream to disk",
  async fn() {
    const file: string = await Deno.makeTempFile();
    const { rid } = await Deno.open(file, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    await Deno.ftruncate(rid, size);

    await new Promise<void>((resolve, reject) => {
      fsync(rid, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        async () => {
          assertEquals((await Deno.stat(file)).size, size);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(async () => {
        await Deno.remove(file);
        Deno.close(rid);
      });
  },
});

Deno.test({
  name: "SYNC: flush any pending data the given file stream to disk",
  fn() {
    const file: string = Deno.makeTempFileSync();
    const { rid } = Deno.openSync(file, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    Deno.ftruncateSync(rid, size);

    try {
      fsyncSync(rid);
      assertEquals(Deno.statSync(file).size, size);
    } finally {
      Deno.removeSync(file);
      Deno.close(rid);
    }
  },
});
