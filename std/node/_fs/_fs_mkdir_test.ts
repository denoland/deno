// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertStringIncludes } from "../../testing/asserts.ts";
import { mkdir, mkdirSync } from "./_fs_mkdir.ts";
import { existsSync } from "./_fs_exists.ts";

const tmpDir = "./tmpdir";

Deno.test({
  name: "[node/fs] mkdir",
  fn: async () => {
    const result = await new Promise((resolve) => {
      mkdir(tmpDir, (err) => {
        err && resolve(false);
        resolve(existsSync(tmpDir));
        Deno.removeSync(tmpDir);
      });
    });
    assert(result);
  },
});

Deno.test({
  name: "[node/fs] mkdirSync",
  fn: () => {
    mkdirSync(tmpDir);
    assert(existsSync(tmpDir));
    Deno.removeSync(tmpDir);
  },
});

Deno.test("[std/node/fs] mkdir callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { mkdir } from "${
        new URL("./_fs_mkdir.ts", import.meta.url).href
      }";

      mkdir(${JSON.stringify(`${tempDir}/subdir`)}, (err) => {
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
