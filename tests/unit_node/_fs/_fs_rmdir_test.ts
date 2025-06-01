// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, fail } from "@std/assert";
import { rmdir, rmdirSync } from "node:fs";
import { existsSync } from "node:fs";
import { join } from "@std/path";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";

Deno.test({
  name: "ASYNC: removing empty folder",
  async fn() {
    const dir = Deno.makeTempDirSync();
    await new Promise<void>((resolve, reject) => {
      rmdir(dir, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(dir), false), () => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir);
      });
  },
});

Deno.test({
  name: "SYNC: removing empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    rmdirSync(dir);
    assertEquals(existsSync(dir), false);
  },
});

Deno.test({
  name: "ASYNC: removing non-empty folder",
  async fn() {
    const dir = Deno.makeTempDirSync();
    using _file1 = Deno.createSync(join(dir, "file1.txt"));
    using _file2 = Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    using _file = Deno.createSync(join(dir, "some_dir", "file.txt"));
    await new Promise<void>((resolve, reject) => {
      rmdir(dir, { recursive: true }, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(dir), false), () => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir, { recursive: true });
      });
  },
  ignore: Deno.build.os === "windows",
});

Deno.test({
  name: "SYNC: removing non-empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    using _file1 = Deno.createSync(join(dir, "file1.txt"));
    using _file2 = Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    using _file = Deno.createSync(join(dir, "some_dir", "file.txt"));
    rmdirSync(dir, { recursive: true });
    assertEquals(existsSync(dir), false);
  },
  ignore: Deno.build.os === "windows",
});

Deno.test("[std/node/fs] rmdir callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertRejects won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { rmdir } from ${JSON.stringify(importUrl)}`,
    invocation: `rmdir(${JSON.stringify(tempDir)}, `,
  });
});
