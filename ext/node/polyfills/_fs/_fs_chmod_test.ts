// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertRejects,
  assertThrows,
  fail,
} from "../../testing/asserts.ts";
import { isWindows } from "../../_util/os.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { chmod, chmodSync } from "./_fs_chmod.ts";

Deno.test({
  name: "ASYNC: Permissions are changed (non-Windows)",
  ignore: isWindows,
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
  name: "ASYNC: don't throw errors for mode parameter (Windows)",
  ignore: !isWindows,
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await new Promise<void>((resolve, reject) => {
      // @ts-ignore for test
      chmod(tempFile, null, (err) => {
        if (err) reject(err);
        else resolve();
      });
    }).finally(() => {
      Deno.removeSync(tempFile);
    });
  },
});

Deno.test({
  name: "ASYNC: don't throw NotSupportedError (Windows)",
  ignore: !isWindows,
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
  ignore: !isWindows,
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
  ignore: isWindows,
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
  name: "SYNC: don't throw errors for mode parameter (Windows)",
  ignore: !isWindows,
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    try {
      // @ts-ignore for test
      chmodSync(tempFile, null);
    } finally {
      Deno.removeSync(tempFile);
    }
  },
});

Deno.test({
  name: "SYNC: don't throw NotSupportedError (Windows)",
  ignore: !isWindows,
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
  ignore: !isWindows,
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
