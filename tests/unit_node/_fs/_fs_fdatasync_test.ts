// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "@std/assert";
import { fdatasync, fdatasyncSync } from "node:fs";

Deno.test({
  name:
    "ASYNC: flush any pending data operations of the given file stream to disk",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
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
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
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
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
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
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      fdatasyncSync(file.rid);
      assertEquals(Deno.readFileSync(filePath), data);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});
