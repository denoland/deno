// Copyright 2018-2025 the Deno authors. MIT license.
import * as path from "@std/path";
import { assert } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { copyFile, copyFileSync, cpSync, existsSync } from "node:fs";

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
  // (assertRejects won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  const tempFile1 = path.join(tempDir, "file1.txt");
  const tempFile2 = path.join(tempDir, "file2.txt");
  await Deno.writeTextFile(tempFile1, "hello world");
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { copyFile } from ${JSON.stringify(importUrl)}`,
    invocation: `copyFile(${JSON.stringify(tempFile1)},
                          ${JSON.stringify(tempFile2)}, `,
    async cleanup() {
      await Deno.remove(tempDir, { recursive: true });
    },
  });
});

Deno.test("[std/node/fs] cp creates destination directory", async () => {
  const tempDir = await Deno.makeTempDir();
  const tempFile1 = path.join(tempDir, "file1.txt");
  const tempFile2 = path.join(tempDir, "dir", "file2.txt");
  await Deno.writeTextFile(tempFile1, "hello world");
  cpSync(tempFile1, tempFile2);
  assert(existsSync(tempFile2));
  await Deno.remove(tempDir, { recursive: true });
});
