// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, fail } from "../../testing/asserts.ts";
import { isWindows } from "../../_util/os.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { chown, chownSync } from "./_fs_chown.ts";

// chown is difficult to test.  Best we can do is set the existing user id/group
// id again
const ignore = isWindows;

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
  ignore: isWindows,
  async fn() {
    const tempFile = await Deno.makeTempFile();
    const { uid, gid } = await Deno.lstat(tempFile);
    const importUrl = new URL("./_fs_chown.ts", import.meta.url);
    await assertCallbackErrorUncaught({
      prelude: `import { chown } from ${JSON.stringify(importUrl)}`,
      invocation: `chown(${JSON.stringify(tempFile)}, ${uid}, ${gid}, `,
      async cleanup() {
        await Deno.remove(tempFile);
      },
    });
  },
});
