// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertRejects,
  assertThrows,
  fail,
} from "../../../../test_util/std/assert/mod.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { chmod, chmodSync } from "node:fs";

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
      }, (error) => {
        fail(error);
      })
      .finally(() => {
        Deno.removeSync(tempFile);
      });
  },
});

Deno.test({
  name: "ASYNC: don't throw NotSupportedError (Windows)",
  ignore: Deno.build.os !== "windows",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await new Promise<void>((resolve, reject) => {
      chmod(tempFile, 0o777, (err) => {
        if (err) reject(err);
        else resolve();
      });
    }).finally(() => {
      Deno.removeSync(tempFile);
    });
  },
});

Deno.test({
  name: "ASYNC: don't swallow NotFoundError (Windows)",
  ignore: Deno.build.os !== "windows",
  async fn() {
    await assertRejects(async () => {
      await new Promise<void>((resolve, reject) => {
        chmod("./__non_existent_file__", 0o777, (err) => {
          if (err) reject(err);
          else resolve();
        });
      });
    });
  },
});

Deno.test({
  name: "SYNC: Permissions are changed (non-Windows)",
  ignore: Deno.build.os === "windows",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    try {
      const originalFileMode: number | null = Deno.lstatSync(tempFile).mode;
      chmodSync(tempFile, "777");

      const newFileMode: number | null = Deno.lstatSync(tempFile).mode;
      assert(newFileMode && originalFileMode);
      assert(newFileMode === 33279 && newFileMode > originalFileMode);
    } finally {
      Deno.removeSync(tempFile);
    }
  },
});

Deno.test({
  name: "SYNC: don't throw NotSupportedError (Windows)",
  ignore: Deno.build.os !== "windows",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    try {
      chmodSync(tempFile, "777");
    } finally {
      Deno.removeSync(tempFile);
    }
  },
});

Deno.test({
  name: "SYNC: don't swallow NotFoundError (Windows)",
  ignore: Deno.build.os !== "windows",
  fn() {
    assertThrows(() => {
      chmodSync("./__non_existent_file__", "777");
    });
  },
});

Deno.test({
  name: "[std/node/fs] chmod callback isn't called twice if error is thrown",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    const importUrl = new URL("node:fs", import.meta.url);
    await assertCallbackErrorUncaught({
      prelude: `import { chmod } from ${JSON.stringify(importUrl)}`,
      invocation: `chmod(${JSON.stringify(tempFile)}, 0o777, `,
      async cleanup() {
        await Deno.remove(tempFile);
      },
    });
  },
});
