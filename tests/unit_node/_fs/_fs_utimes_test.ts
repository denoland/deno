// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, assertThrows, fail } from "@std/assert";
import { utimes, utimesSync } from "node:fs";

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

Deno.test({
  name: "SYNC and ASYNC: utimes on read-only Windows files",
  ignore: Deno.build.os !== "windows",
  async fn() {
    const file = Deno.makeTempFileSync();
    try {
      Deno.chmodSync(file, 0o444);
      const syncTime = new Date(50_000_000);
      utimesSync(file, syncTime, syncTime);
      assertEquals(Deno.statSync(file).mtime, syncTime);

      const asyncTime = new Date(60_000_000);
      await new Promise<void>((resolve, reject) => {
        utimes(file, asyncTime, asyncTime, (err) => {
          if (err) reject(err);
          else resolve();
        });
      });
      assertEquals(Deno.statSync(file).mtime, asyncTime);
    } finally {
      Deno.chmodSync(file, 0o666);
      Deno.removeSync(file);
    }
  },
});
