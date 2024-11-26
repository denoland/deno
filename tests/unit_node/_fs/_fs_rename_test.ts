// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { rename, renameSync } from "node:fs";
import { existsSync } from "node:fs";
import { join, parse } from "@std/path";

Deno.test({
  name: "ASYNC: renaming a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    await new Promise<void>((resolve, reject) => {
      rename(file, newPath, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => {
        assertEquals(existsSync(newPath), true);
        assertEquals(existsSync(file), false);
      }, () => fail())
      .finally(() => {
        if (existsSync(file)) Deno.removeSync(file);
        if (existsSync(newPath)) Deno.removeSync(newPath);
      });
  },
});

Deno.test({
  name: "SYNC: renaming a file",
  fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    renameSync(file, newPath);
    assertEquals(existsSync(newPath), true);
    assertEquals(existsSync(file), false);
  },
});

Deno.test("[std/node/fs] rename callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { rename } from ${JSON.stringify(importUrl)}`,
    invocation: `rename(${JSON.stringify(tempFile)},
                        ${JSON.stringify(`${tempFile}.newname`)}, `,
    async cleanup() {
      await Deno.remove(`${tempFile}.newname`);
    },
  });
});
