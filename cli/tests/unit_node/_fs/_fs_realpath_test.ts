// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as path from "../../path/mod.ts";
import { assertEquals } from "../../testing/asserts.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { realpath, realpathSync } from "./_fs_realpath.ts";

Deno.test("realpath", async function () {
  const tempFile = await Deno.makeTempFile();
  const tempFileAlias = tempFile + ".alias";
  await Deno.symlink(tempFile, tempFileAlias);
  const realPath = await new Promise((resolve, reject) => {
    realpath(tempFile, (err, path) => {
      if (err) {
        reject(err);
        return;
      }
      resolve(path);
    });
  });
  const realSymLinkPath = await new Promise((resolve, reject) => {
    realpath(tempFileAlias, (err, path) => {
      if (err) {
        reject(err);
        return;
      }
      resolve(path);
    });
  });
  assertEquals(realPath, realSymLinkPath);
});

Deno.test("realpathSync", function () {
  const tempFile = Deno.makeTempFileSync();
  const tempFileAlias = tempFile + ".alias";
  Deno.symlinkSync(tempFile, tempFileAlias);
  const realPath = realpathSync(tempFile);
  const realSymLinkPath = realpathSync(tempFileAlias);
  assertEquals(realPath, realSymLinkPath);
});

Deno.test("[std/node/fs] realpath callback isn't called twice if error is thrown", async () => {
  const tempDir = await Deno.makeTempDir();
  const tempFile = path.join(tempDir, "file.txt");
  const linkFile = path.join(tempDir, "link.txt");
  await Deno.writeTextFile(tempFile, "hello world");
  await Deno.symlink(tempFile, linkFile, { type: "file" });
  const importUrl = new URL("./_fs_realpath.ts", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { realpath } from ${JSON.stringify(importUrl)}`,
    invocation: `realpath(${JSON.stringify(`${tempDir}/link.txt`)}, `,
    async cleanup() {
      await Deno.remove(tempDir, { recursive: true });
    },
  });
});
