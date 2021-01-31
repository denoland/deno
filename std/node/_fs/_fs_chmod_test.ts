// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, fail } from "../../testing/asserts.ts";
import { assertCallbackErrorUncaught } from "../_utils.ts";
import { chmod, chmodSync } from "./_fs_chmod.ts";

Deno.test({
  name: "ASYNC: Permissions are changed (non-Windows)",
  ignore: Deno.build.os === "windows",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const originalFileMode: number | null = (await Deno.lstat(tempFile)).mode;
    await new Promise<void>((resolve, reject) => {
      chmod(tempFile, 0o777, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(() => {
        const newFileMode: number | null = Deno.lstatSync(tempFile).mode;
        assert(newFileMode && originalFileMode);
        assert(newFileMode === 33279 && newFileMode > originalFileMode);
      }, () => {
        fail();
      })
      .finally(() => {
        Deno.removeSync(tempFile);
      });
  },
});

Deno.test({
  name: "SYNC: Permissions are changed (non-Windows)",
  ignore: Deno.build.os === "windows",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const originalFileMode: number | null = Deno.lstatSync(tempFile).mode;
    chmodSync(tempFile, "777");

    const newFileMode: number | null = Deno.lstatSync(tempFile).mode;
    assert(newFileMode && originalFileMode);
    assert(newFileMode === 33279 && newFileMode > originalFileMode);
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "[std/node/fs] chmod callback isn't called twice if error is thrown",
  ignore: Deno.build.os === "windows",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    const importUrl = new URL("./_fs_chmod.ts", import.meta.url);
    await assertCallbackErrorUncaught({
      prelude: `import { chmod } from ${JSON.stringify(importUrl)}`,
      invocation: `chmod(${JSON.stringify(tempFile)}, 0o777, `,
      async cleanup() {
        await Deno.remove(tempFile);
      },
    });
  },
});
