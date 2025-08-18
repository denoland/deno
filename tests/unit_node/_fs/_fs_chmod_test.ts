// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, assertRejects, assertThrows, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { chmod, chmodSync } from "node:fs";

let modeAsync: number;
let modeSync: number;
// On Windows chmod is only able to manipulate write permission
if (Deno.build.os === "windows") {
  modeAsync = 0o444; // read-only
  modeSync = 0o666; // read-write
} else {
  modeAsync = 0o777;
  modeSync = 0o644;
}

Deno.test({
  name: "ASYNC: Permissions are changed",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await new Promise<void>((resolve, reject) => {
      chmod(tempFile, modeAsync, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(() => {
        const fileMode = Deno.lstatSync(tempFile).mode as number;
        assertEquals(fileMode & 0o777, modeAsync);
      }, (error) => {
        fail(error);
      })
      .finally(() => {
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
  name: "SYNC: Permissions are changed",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    try {
      chmodSync(tempFile, modeSync.toString(8));

      const fileMode = Deno.lstatSync(tempFile).mode as number;
      assertEquals(fileMode & 0o777, modeSync);
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
