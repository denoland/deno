// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, fail } from "@std/assert";
import { futimes, futimesSync } from "node:fs";

const randomDate = new Date(Date.now() + 1000);

Deno.test({
  name:
    "ASYNC: change the file system timestamps of the object referenced by path",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = Deno.makeTempFileSync();
    using file = await Deno.open(filePath, { create: true, write: true });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      futimes(file.rid, randomDate, randomDate, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
          assertEquals(fileInfo.mtime, randomDate);
          assertEquals(fileInfo.atime, randomDate);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => {
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "ASYNC: should throw error if atime is infinity",
  fn() {
    assertThrows(
      () => {
        futimes(123, Infinity, 0, (_err: Error | null) => {});
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});

Deno.test({
  name: "ASYNC: should throw error if atime is NaN",
  fn() {
    assertThrows(
      () => {
        futimes(123, "some string", 0, (_err: Error | null) => {});
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});

Deno.test({
  name:
    "SYNC: change the file system timestamps of the object referenced by path",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath, { create: true, write: true });

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      futimesSync(file.rid, randomDate, randomDate);

      const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);

      assertEquals(fileInfo.mtime, randomDate);
      assertEquals(fileInfo.atime, randomDate);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

Deno.test({
  name: "SYNC: should throw error if atime is NaN",
  fn() {
    assertThrows(
      () => {
        futimesSync(123, "some string", 0);
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});

Deno.test({
  name: "SYNC: should throw error if atime is Infinity",
  fn() {
    assertThrows(
      () => {
        futimesSync(123, Infinity, 0);
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});
