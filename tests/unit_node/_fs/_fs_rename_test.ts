// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { rename, renameSync } from "node:fs";
import { existsSync } from "node:fs";
import { join, parse } from "@std/path";
import { Buffer } from "node:buffer";

Deno.test({
  name: "ASYNC: renaming a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    await new Promise<void>((resolve, reject) => {
      rename(file, newPath, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => {
        assertEquals(existsSync(newPath), true);
        assertEquals(existsSync(file), false);
      }, () => fail())
      .finally(() => {
        if (existsSync(file)) Deno.removeSync(file);
        if (existsSync(newPath)) Deno.removeSync(newPath);
      });
  },
});

Deno.test({
  name: "SYNC: renaming a file",
  fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    renameSync(file, newPath);
    assertEquals(existsSync(newPath), true);
    assertEquals(existsSync(file), false);
  },
});

Deno.test("[std/node/fs] rename callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { rename } from ${JSON.stringify(importUrl)}`,
    invocation: `rename(${JSON.stringify(tempFile)},
                        ${JSON.stringify(`${tempFile}.newname`)}, `,
    async cleanup() {
      await Deno.remove(`${tempFile}.newname`);
    },
  });
});

Deno.test("[std/node/fs] rename: accepts Buffer paths", async () => {
  const file = Deno.makeTempFileSync();
  const bufferOldPath = Buffer.from(file, "utf-8");
  const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
  const bufferNewPath = Buffer.from(newPath, "utf-8");

  await new Promise<void>((resolve, reject) => {
    rename(bufferOldPath, bufferNewPath, (err) => {
      if (err) reject(err);
      resolve();
    });
  })
    .then(() => {
      assertEquals(existsSync(newPath), true);
      assertEquals(existsSync(file), false);
    }, () => fail());
});

Deno.test("[std/node/fs] rename: convert Deno errors to Node.js errors", async () => {
  const dir = Deno.makeTempDirSync();
  const oldPath = join(dir, "non_existent_file");
  const newPath = join(dir, "new_file");

  await new Promise<void>((resolve, reject) => {
    rename(oldPath, newPath, (err) => {
      if (err) reject(err);
      resolve();
    });
  })
    .then(() => fail())
    .catch((err) => {
      assertEquals(err.code, "ENOENT");
      assertEquals(err.syscall, "rename");
      assertEquals(err.path, oldPath);
      assertEquals(err.dest, newPath);
    });
});

Deno.test("[std/node/fs] renameSync: accepts Buffer paths", () => {
  const file = Deno.makeTempFileSync();
  const bufferOldPath = Buffer.from(file, "utf-8");
  const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
  const bufferNewPath = Buffer.from(newPath, "utf-8");

  renameSync(bufferOldPath, bufferNewPath);
  assertEquals(existsSync(newPath), true);
  assertEquals(existsSync(file), false);
});

Deno.test("[std/node/fs] renameSync: convert Deno errors to Node.js errors", () => {
  const dir = Deno.makeTempDirSync();
  const oldPath = join(dir, "non_existent_file");
  const newPath = join(dir, "new_file");

  try {
    renameSync(oldPath, newPath);
    fail();
  } catch (err) {
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).code, "ENOENT");
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).syscall, "rename");
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).path, oldPath);
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).dest, newPath);
  }
});
