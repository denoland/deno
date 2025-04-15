// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, assertThrows, fail } from "@std/assert";
import { truncate, truncateSync } from "node:fs";

Deno.test({
  name: "ASYNC: no callback function results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Argument of type 'number' is not assignable to parameter of type 'NoParamCallback'
        truncate("some/path", 0);
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "ASYNC: truncate entire file contents",
  async fn() {
    const file: string = Deno.makeTempFileSync();
    await Deno.writeTextFile(file, "hello world");

    await new Promise<void>((resolve, reject) => {
      truncate(file, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(file);
          assertEquals(fileInfo.size, 0);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "ASYNC: truncate file to a size of precisely len bytes",
  async fn() {
    const file: string = Deno.makeTempFileSync();
    await Deno.writeTextFile(file, "hello world");

    await new Promise<void>((resolve, reject) => {
      truncate(file, 3, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(file);
          assertEquals(fileInfo.size, 3);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: truncate entire file contents",
  fn() {
    const file: string = Deno.makeTempFileSync();
    try {
      truncateSync(file);
      const fileInfo: Deno.FileInfo = Deno.lstatSync(file);
      assertEquals(fileInfo.size, 0);
    } finally {
      Deno.removeSync(file);
    }
  },
});

Deno.test({
  name: "SYNC: truncate file to a size of precisely len bytes",
  fn() {
    const file: string = Deno.makeTempFileSync();
    try {
      truncateSync(file, 3);
      const fileInfo: Deno.FileInfo = Deno.lstatSync(file);
      assertEquals(fileInfo.size, 3);
    } finally {
      Deno.removeSync(file);
    }
  },
});
