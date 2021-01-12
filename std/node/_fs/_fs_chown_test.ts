// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  fail,
} from "../../testing/asserts.ts";
import { chown, chownSync } from "./_fs_chown.ts";

// chown is difficult to test.  Best we can do is set the existing user id/group
// id again
const ignore = Deno.build.os == "windows";

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
    // The correct behaviour is not to catch any errors thrown,
    // but that means there'll be an uncaught error and the test will fail.
    // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
    // (assertThrowsAsync won't work because there's no way to catch the error.)
    const tempFile = await Deno.makeTempFile();
    const { uid, gid } = await Deno.lstat(tempFile);
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        "--no-check",
        `
        import { chown } from "${
          new URL("./_fs_chown.ts", import.meta.url).href
        }";

        chown(${JSON.stringify(tempFile)}, ${uid}, ${gid}, (err) => {
          // If the bug is present and the callback is called again with an error,
          // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
          if (!err) throw new Error("success");
        });`,
      ],
      stderr: "piped",
    });
    const status = await p.status();
    const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
    p.close();
    p.stderr.close();
    await Deno.remove(tempFile);
    assert(!status.success);
    assertStringIncludes(stderr, "Error: success");
  },
});
