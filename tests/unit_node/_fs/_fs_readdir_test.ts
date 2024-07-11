// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertNotEquals, fail } from "@std/assert/mod.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { Dirent, readdir, readdirSync } from "node:fs";
import { join } from "@std/path/mod.ts";

Deno.test({
  name: "ASYNC: reading empty directory",
  async fn() {
    const dir = Deno.makeTempDirSync();
    await new Promise<string[]>((resolve, reject) => {
      readdir(dir, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    })
      .then((files) => assertEquals(files, []), () => fail())
      .finally(() => Deno.removeSync(dir));
  },
});

function assertEqualsArrayAnyOrder<T>(actual: T[], expected: T[]) {
  assertEquals(actual.length, expected.length);
  for (const item of expected) {
    const index = actual.indexOf(item);
    assertNotEquals(index, -1);
    expected = expected.splice(index, 1);
  }
}

Deno.test({
  name: "ASYNC: reading non-empty directory",
  async fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    await new Promise<string[]>((resolve, reject) => {
      readdir(dir, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    })
      .then(
        (files) =>
          assertEqualsArrayAnyOrder(
            files,
            ["file1.txt", "some_dir", "file2.txt"],
          ),
        () => fail(),
      )
      .finally(() => Deno.removeSync(dir, { recursive: true }));
  },
});

Deno.test({
  name: "SYNC: reading empty the directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    assertEquals(readdirSync(dir), []);
  },
});

Deno.test({
  name: "SYNC: reading non-empty directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    assertEqualsArrayAnyOrder(
      readdirSync(dir),
      ["file1.txt", "some_dir", "file2.txt"],
    );
  },
});

Deno.test("[std/node/fs] readdir callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertRejects won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { readdir } from ${JSON.stringify(importUrl)}`,
    invocation: `readdir(${JSON.stringify(tempDir)}, `,
    async cleanup() {
      await Deno.remove(tempDir);
    },
  });
});

Deno.test("[node/fs] readdir creates Dirent instances", async () => {
  const tempDir = await Deno.makeTempDir();
  await Deno.writeTextFile(join(tempDir, "file.txt"), "file content");
  await Deno.symlink(join(tempDir, "file.txt"), join(tempDir, "link"));
  await Deno.mkdir(join(tempDir, "dir"));

  try {
    const result = await new Promise<Dirent[]>((resolve, reject) => {
      readdir(tempDir, { withFileTypes: true }, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    });
    result.sort((a, b) => a.name.localeCompare(b.name));

    assertEquals(result[0].name, "dir");
    assertEquals(result[0].isFile(), false);
    assertEquals(result[0].isDirectory(), true);
    assertEquals(result[0].isSymbolicLink(), false);

    assertEquals(result[1].name, "file.txt");
    assertEquals(result[1].isFile(), true);
    assertEquals(result[1].isDirectory(), false);
    assertEquals(result[1].isSymbolicLink(), false);

    assertEquals(result[2].name, "link");
    assertEquals(result[2].isFile(), false);
    assertEquals(result[2].isDirectory(), false);
    assertEquals(result[2].isSymbolicLink(), true);
  } finally {
    await Deno.remove(tempDir, { recursive: true });
  }
});

Deno.test("[node/fs] readdirSync creates Dirent instances", async () => {
  const tempDir = await Deno.makeTempDir();
  await Deno.writeTextFile(join(tempDir, "file.txt"), "file content");
  await Deno.symlink(join(tempDir, "file.txt"), join(tempDir, "link"));
  await Deno.mkdir(join(tempDir, "dir"));

  try {
    const result = readdirSync(tempDir, { withFileTypes: true });
    result.sort((a, b) => a.name.localeCompare(b.name));

    assertEquals(result[0].name, "dir");
    assertEquals(result[0].isFile(), false);
    assertEquals(result[0].isDirectory(), true);
    assertEquals(result[0].isSymbolicLink(), false);

    assertEquals(result[1].name, "file.txt");
    assertEquals(result[1].isFile(), true);
    assertEquals(result[1].isDirectory(), false);
    assertEquals(result[1].isSymbolicLink(), false);

    assertEquals(result[2].name, "link");
    assertEquals(result[2].isFile(), false);
    assertEquals(result[2].isDirectory(), false);
    assertEquals(result[2].isSymbolicLink(), true);
  } finally {
    await Deno.remove(tempDir, { recursive: true });
  }
});
