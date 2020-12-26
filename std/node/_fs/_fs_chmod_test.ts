// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertStringIncludes, fail } from "../../testing/asserts.ts";
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
    // The correct behaviour is not to catch any errors thrown,
    // but that means there'll be an uncaught error and the test will fail.
    // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
    // (assertThrowsAsync won't work because there's no way to catch the error.)
    const tempFile = await Deno.makeTempFile();
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        "--no-check",
        `
        import { chmod } from "${
          new URL("./_fs_chmod.ts", import.meta.url).href
        }";

        chmod(${JSON.stringify(tempFile)}, 0o777, (err) => {
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
