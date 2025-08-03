// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { existsSync, unlink, unlinkSync } from "node:fs";
import { Buffer } from "node:buffer";
import { join } from "@std/path";

Deno.test({
  name: "ASYNC: deleting a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<void>((resolve, reject) => {
      unlink(file, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(file), false), () => fail())
      .finally(() => {
        if (existsSync(file)) Deno.removeSync(file);
      });
  },
});

Deno.test({
  name: "SYNC: Test deleting a file",
  fn() {
    const file = Deno.makeTempFileSync();
    unlinkSync(file);
    assertEquals(existsSync(file), false);
  },
});

Deno.test("[std/node/fs] unlink callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { unlink } from ${JSON.stringify(importUrl)}`,
    invocation: `unlink(${JSON.stringify(tempFile)}, `,
  });
});

Deno.test("[std/node/fs] unlink accepts Buffer path", async () => {
  const file = Deno.makeTempFileSync();
  const bufferPath = Buffer.from(file, "utf-8");
  await new Promise<void>((resolve, reject) => {
    unlink(bufferPath, (err) => {
      if (err) reject(err);
      resolve();
    });
  })
    .then(() => assertEquals(existsSync(file), false), () => fail());
});

Deno.test("[std/node/fs] unlinkSync accepts Buffer path", () => {
  const file = Deno.makeTempFileSync();
  const bufferPath = Buffer.from(file, "utf-8");
  unlinkSync(bufferPath);
  assertEquals(existsSync(file), false);
});

Deno.test("[std/node/fs] unlink: convert Deno error to Node.js error", async () => {
  const dir = Deno.makeTempDirSync();
  const path = join(dir, "non_existent_file");

  await new Promise<void>((resolve, reject) => {
    unlink(path, (err) => {
      if (err) reject(err);
      resolve();
    });
  })
    .then(() => fail(), (err) => {
      assertEquals(err.code, "ENOENT");
      assertEquals(err.syscall, "unlink");
      assertEquals(err.path, path);
    });
});

Deno.test("[std/node/fs] unlinkSync: convert Deno error to Node.js error", () => {
  const dir = Deno.makeTempDirSync();
  const path = join(dir, "non_existent_file");

  try {
    unlinkSync(path);
    fail();
  } catch (err) {
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).code, "ENOENT");
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).syscall, "unlink");
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).path, path);
  }
});
