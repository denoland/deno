// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "@std/assert";
import { existsSync } from "node:fs";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { unlink, unlinkSync } from "node:fs";

Deno.test({
  name: "ASYNC: deleting a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<void>((resolve, reject) => {
      unlink(file, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(file), false), () => fail())
      .finally(() => {
        if (existsSync(file)) Deno.removeSync(file);
      });
  },
});

Deno.test({
  name: "SYNC: Test deleting a file",
  fn() {
    const file = Deno.makeTempFileSync();
    unlinkSync(file);
    assertEquals(existsSync(file), false);
  },
});

Deno.test("[std/node/fs] unlink callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { unlink } from ${JSON.stringify(importUrl)}`,
    invocation: `unlink(${JSON.stringify(tempFile)}, `,
  });
});
