// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, assertNotEquals, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { type Dirent, readdir, readdirSync } from "node:fs";
import { Buffer } from "node:buffer";
import { join } from "@std/path";

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

Deno.test("ASYNC: read dirs recursively", async () => {
  const dir = Deno.makeTempDirSync();
  Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
  Deno.mkdirSync(join(dir, "sub"));
  Deno.writeTextFileSync(join(dir, "sub", "file2.txt"), "hi");

  try {
    const files = await new Promise<string[]>((resolve, reject) => {
      readdir(dir, { recursive: true }, (err, files) => {
        if (err) reject(err);
        resolve(files.map((f) => f.toString()));
      });
    });

    assertEqualsArrayAnyOrder(
      files,
      ["file1.txt", "sub", join("sub", "file2.txt")],
    );
  } finally {
    Deno.removeSync(dir, { recursive: true });
  }
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

Deno.test("SYNC: read dirs recursively", () => {
  const dir = Deno.makeTempDirSync();
  Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
  Deno.mkdirSync(join(dir, "sub"));
  Deno.writeTextFileSync(join(dir, "sub", "file2.txt"), "hi");

  try {
    const files = readdirSync(dir, { recursive: true }).map((f) =>
      f.toString()
    );

    assertEqualsArrayAnyOrder(
      files,
      ["file1.txt", "sub", join("sub", "file2.txt")],
    );
  } finally {
    Deno.removeSync(dir, { recursive: true });
  }
});

Deno.test("SYNC: withFileTypes + buffer encoding returns Buffer name/parentPath", () => {
  const dir = Deno.makeTempDirSync();
  try {
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    const entries = readdirSync(Buffer.from(dir) as unknown as string, {
      withFileTypes: true,
      encoding: "buffer",
    } as unknown as { withFileTypes: true }) as unknown as Dirent[];
    assertEquals(entries.length, 1);
    const [entry] = entries;
    assertEquals(Buffer.isBuffer(entry.name), true);
    assertEquals(
      (entry.name as unknown as Buffer).toString("utf8"),
      "file1.txt",
    );
    assertEquals(Buffer.isBuffer(entry.parentPath), true);
    assertEquals(
      (entry.parentPath as unknown as Buffer).toString("utf8"),
      dir,
    );
  } finally {
    Deno.removeSync(dir, { recursive: true });
  }
});

Deno.test("ASYNC: withFileTypes + buffer encoding returns Buffer name/parentPath", async () => {
  const dir = Deno.makeTempDirSync();
  try {
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    const entries = await new Promise<Dirent[]>((resolve, reject) => {
      readdir(
        Buffer.from(dir) as unknown as string,
        {
          withFileTypes: true,
          encoding: "buffer",
        } as unknown as { withFileTypes: true },
        (err, files) => {
          if (err) reject(err);
          else resolve(files as unknown as Dirent[]);
        },
      );
    });
    assertEquals(entries.length, 1);
    const [entry] = entries;
    assertEquals(Buffer.isBuffer(entry.name), true);
    assertEquals(
      (entry.name as unknown as Buffer).toString("utf8"),
      "file1.txt",
    );
    assertEquals(Buffer.isBuffer(entry.parentPath), true);
    assertEquals(
      (entry.parentPath as unknown as Buffer).toString("utf8"),
      dir,
    );
  } finally {
    Deno.removeSync(dir, { recursive: true });
  }
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
