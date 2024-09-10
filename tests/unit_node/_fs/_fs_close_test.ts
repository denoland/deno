// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { close, closeSync } from "node:fs";

Deno.test({
  name: "ASYNC: File is closed",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const file: Deno.FsFile = await Deno.open(tempFile);

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      close(file.rid, (err) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .catch(() => fail("No error expected"))
      .finally(async () => {
        await Deno.remove(tempFile);
      });
  },
});

Deno.test({
  name: "ASYNC: Invalid fd",
  fn() {
    assertThrows(() => {
      close(-1, (_err) => {});
    }, RangeError);
  },
});

Deno.test({
  name: "close callback should be asynchronous",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const file: Deno.FsFile = Deno.openSync(tempFile);

    let foo: string;
    const promise = new Promise<void>((resolve) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      close(file.rid, () => {
        assert(foo === "bar");
        resolve();
      });
      foo = "bar";
    });

    await promise;
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "SYNC: File is closed",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const file: Deno.FsFile = Deno.openSync(tempFile);

    // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
    closeSync(file.rid);
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "SYNC: Invalid fd",
  fn() {
    assertThrows(() => closeSync(-1));
  },
});

Deno.test({
  name: "[std/node/fs] close callback isn't called twice if error is thrown",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
}, async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `
    import { close } from ${JSON.stringify(importUrl)};

    const file = await Deno.open(${JSON.stringify(tempFile)});
    `,
    invocation: "close(file.rid, ",
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});
