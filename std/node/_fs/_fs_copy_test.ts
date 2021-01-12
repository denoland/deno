// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertStringIncludes } from "../../testing/asserts.ts";
import { copyFile, copyFileSync } from "./_fs_copy.ts";
import { existsSync } from "./_fs_exists.ts";

const destFile = "./destination.txt";

Deno.test({
  name: "[std/node/fs] copy file",
  fn: async () => {
    const sourceFile = Deno.makeTempFileSync();
    const err = await new Promise((resolve) => {
      copyFile(sourceFile, destFile, (err?: Error | null) => resolve(err));
    });
    assert(!err);
    assert(existsSync(destFile));
    Deno.removeSync(sourceFile);
    Deno.removeSync(destFile);
  },
});

Deno.test({
  name: "[std/node/fs] copy file sync",
  fn: () => {
    const sourceFile = Deno.makeTempFileSync();
    copyFileSync(sourceFile, destFile);
    assert(existsSync(destFile));
    Deno.removeSync(sourceFile);
    Deno.removeSync(destFile);
  },
});

Deno.test("[std/node/fs] copyFile callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  await Deno.writeTextFile(`${tempDir}/file1.txt`, "hello world");
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { copyFile } from "${
        new URL("./_fs_copy.ts", import.meta.url).href
      }";

      copyFile(${JSON.stringify(`${tempDir}/file1.txt`)}, ${
        JSON.stringify(`${tempDir}/file2.txt`)
      }, (err) => {
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
  await Deno.remove(tempDir, { recursive: true });
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
