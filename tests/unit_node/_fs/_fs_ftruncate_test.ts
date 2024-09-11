// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, fail } from "@std/assert";
import { ftruncate, ftruncateSync } from "node:fs";

Deno.test({
  name: "ASYNC: no callback function results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Argument of type 'number' is not assignable to parameter of type 'NoParamCallback'
        ftruncate(123, 0);
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "ASYNC: truncate entire file contents",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = Deno.makeTempFileSync();
    await Deno.writeTextFile(filePath, "hello world");
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncate(file.rid, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
          assertEquals(fileInfo.size, 0);
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
  name: "ASYNC: truncate file to a size of precisely len bytes",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = Deno.makeTempFileSync();
    await Deno.writeTextFile(filePath, "hello world");
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncate(file.rid, 3, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
          assertEquals(fileInfo.size, 3);
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
  name: "SYNC: truncate entire file contents",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    Deno.writeFileSync(filePath, new TextEncoder().encode("hello world"));
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncateSync(file.rid);
      const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
      assertEquals(fileInfo.size, 0);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

Deno.test({
  name: "SYNC: truncate file to a size of precisely len bytes",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    Deno.writeFileSync(filePath, new TextEncoder().encode("hello world"));
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncateSync(file.rid, 3);
      const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
      assertEquals(fileInfo.size, 3);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});
