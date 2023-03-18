// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, fail } from "../../testing/asserts.ts";
import { utimes, utimesSync } from "./_fs_utimes.ts";

const randomDate = new Date(Date.now() + 1000);

Deno.test({
  name:
    "ASYNC: change the file system timestamps of the object referenced by path",
  async fn() {
    const file: string = Deno.makeTempFileSync();

    await new Promise<void>((resolve, reject) => {
      utimes(file, randomDate, randomDate, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(file);
          assertEquals(fileInfo.mtime, randomDate);
          assertEquals(fileInfo.mtime, randomDate);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "ASYNC: should throw error if atime is infinity",
  fn() {
    assertThrows(
      () => {
        utimes("some/path", Infinity, 0, (_err: Error | null) => {});
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
        utimes("some/path", "some string", 0, (_err: Error | null) => {});
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
    try {
      utimesSync(file, randomDate, randomDate);

      const fileInfo: Deno.FileInfo = Deno.lstatSync(file);

      assertEquals(fileInfo.mtime, randomDate);
    } finally {
      Deno.removeSync(file);
    }
  },
});

Deno.test({
  name: "SYNC: should throw error if atime is NaN",
  fn() {
    assertThrows(
      () => {
        utimesSync("some/path", "some string", 0);
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
        utimesSync("some/path", Infinity, 0);
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});
