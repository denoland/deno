// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { chown, chownSync } from "node:fs";

// chown is difficult to test.  Best we can do is set the existing user id/group
// id again
const ignore = Deno.build.os === "windows";

Deno.test({
  ignore,
  name: "ASYNC: setting existing uid/gid works as expected (non-Windows)",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const originalUserId: number | null = (await Deno.lstat(tempFile)).uid;
    const originalGroupId: number | null = (await Deno.lstat(tempFile)).gid;
    await new Promise<void>((resolve, reject) => {
      chown(tempFile, originalUserId!, originalGroupId!, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(() => {
        const newUserId: number | null = Deno.lstatSync(tempFile).uid;
        const newGroupId: number | null = Deno.lstatSync(tempFile).gid;
        assertEquals(newUserId, originalUserId);
        assertEquals(newGroupId, originalGroupId);
      }, () => {
        fail();
      })
      .finally(() => {
        Deno.removeSync(tempFile);
      });
  },
});

Deno.test({
  ignore,
  name: "SYNC: setting existing uid/gid works as expected (non-Windows)",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const originalUserId: number | null = Deno.lstatSync(tempFile).uid;
    const originalGroupId: number | null = Deno.lstatSync(tempFile).gid;
    chownSync(tempFile, originalUserId!, originalGroupId!);

    const newUserId: number | null = Deno.lstatSync(tempFile).uid;
    const newGroupId: number | null = Deno.lstatSync(tempFile).gid;
    assertEquals(newUserId, originalUserId);
    assertEquals(newGroupId, originalGroupId);
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "[std/node/fs] chown callback isn't called twice if error is thrown",
  ignore: Deno.build.os === "windows",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    const { uid, gid } = await Deno.lstat(tempFile);
    const importUrl = new URL("node:fs", import.meta.url);
    await assertCallbackErrorUncaught({
      prelude: `import { chown } from ${JSON.stringify(importUrl)}`,
      invocation: `chown(${JSON.stringify(tempFile)}, ${uid}, ${gid}, `,
      async cleanup() {
        await Deno.remove(tempFile);
      },
    });
  },
});
