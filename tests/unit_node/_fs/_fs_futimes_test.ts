// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, assertThrows, fail } from "@std/assert";
import { closeSync, futimes, futimesSync, openSync } from "node:fs";

const randomDate = new Date(Date.now() + 1000);

Deno.test({
  name:
    "ASYNC: change the file system timestamps of the object referenced by path",
  async fn() {
    const filePath = Deno.makeTempFileSync();
    const fd = openSync(filePath, "w");

    await new Promise<void>((resolve, reject) => {
      futimes(fd, randomDate, randomDate, (err: Error | null) => {
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
        closeSync(fd);
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
  fn() {
    const filePath = Deno.makeTempFileSync();
    const fd = openSync(filePath, "w");

    try {
      futimesSync(fd, randomDate, randomDate);

      const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);

      assertEquals(fileInfo.mtime, randomDate);
      assertEquals(fileInfo.atime, randomDate);
    } finally {
      closeSync(fd);
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
