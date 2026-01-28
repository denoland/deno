// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertThrows, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { close, closeSync, openSync } from "node:fs";
import { setTimeout } from "node:timers/promises";

Deno.test({
  name: "ASYNC: File is closed",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const fd = openSync(tempFile, "r");

    await new Promise<void>((resolve, reject) => {
      close(fd, (err) => {
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
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const fd = openSync(tempFile, "r");

    let foo: string;
    const promise = new Promise<void>((resolve) => {
      close(fd, () => {
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
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const fd = openSync(tempFile, "r");

    closeSync(fd);
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
}, async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `
    import { close, openSync } from ${JSON.stringify(importUrl)};

    const fd = openSync(${JSON.stringify(tempFile)}, "r");
    `,
    invocation: "close(fd, ",
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});

Deno.test({
  name: "[std/node/fs] close with default callback if none is provided",
}, async () => {
  const tempFile = await Deno.makeTempFile();
  const fd = openSync(tempFile, "r");
  close(fd);
  await setTimeout(1000);
  assertThrows(() => {
    closeSync(fd), Deno.errors.BadResource;
  });
  await Deno.remove(tempFile);
});
