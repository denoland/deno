// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, fail } from "../../testing/asserts.ts";
import { futimes, futimesSync } from "./_fs_futimes.ts";

const randomDate = new Date(Date.now() + 1000);

Deno.test({
  name:
    "ASYNC: change the file system timestamps of the object referenced by path",
  async fn() {
    const file: string = Deno.makeTempFileSync();
    const { rid } = await Deno.open(file, { create: true, write: true });

    await new Promise<void>((resolve, reject) => {
      futimes(rid, randomDate, randomDate, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(file);
          assertEquals(fileInfo.mtime, randomDate);
          assertEquals(fileInfo.atime, randomDate);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => {
        Deno.removeSync(file);
        Deno.close(rid);
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
  fn() {
    const file: string = Deno.makeTempFileSync();
    const { rid } = Deno.openSync(file, { create: true, write: true });

    try {
      futimesSync(rid, randomDate, randomDate);

      const fileInfo: Deno.FileInfo = Deno.lstatSync(file);

      assertEquals(fileInfo.mtime, randomDate);
      assertEquals(fileInfo.atime, randomDate);
    } finally {
      Deno.removeSync(file);
      Deno.close(rid);
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
