// Copyright 2018-2026 the Deno authors. MIT license.
import * as path from "@std/path";
import { assert, assertEquals } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { existsSync, mkdir, mkdirSync } from "node:fs";
import { mkdir as mkdirPromise } from "node:fs/promises";

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

Deno.test({
  name: "[node/fs] mkdir mode",
  fn: () => {
    mkdirSync(tmpDir, { mode: 0o777 });
    assert(existsSync(tmpDir));
    assert(Deno.statSync(tmpDir).mode! & 0o777);

    Deno.removeSync(tmpDir);

    mkdirSync(tmpDir, { mode: "0777" });
    assert(existsSync(tmpDir));
    assert(Deno.statSync(tmpDir).mode! & 0o777);

    Deno.removeSync(tmpDir);
  },
});

Deno.test({
  name: "[node/fs] mkdir recursive returns first created directory (callback)",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const nested = path.join(tempDir, "a", "b", "c");
    const result = await new Promise<string | undefined>((resolve, reject) => {
      mkdir(nested, { recursive: true }, (err, path) => {
        if (err) reject(err);
        else resolve(path);
      });
    });
    assertEquals(result, path.join(tempDir, "a"));
    await Deno.remove(tempDir, { recursive: true });
  },
});

Deno.test({
  name:
    "[node/fs] mkdir recursive returns undefined when dir exists (callback)",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const result = await new Promise<string | undefined>((resolve, reject) => {
      mkdir(tempDir, { recursive: true }, (err, path) => {
        if (err) reject(err);
        else resolve(path);
      });
    });
    assertEquals(result, undefined);
    await Deno.remove(tempDir, { recursive: true });
  },
});

Deno.test({
  name: "[node/fs] mkdirSync recursive returns first created directory",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const nested = path.join(tempDir, "a", "b", "c");
    const result = mkdirSync(nested, { recursive: true });
    assertEquals(result, path.join(tempDir, "a"));
    await Deno.remove(tempDir, { recursive: true });
  },
});

Deno.test({
  name: "[node/fs] mkdirSync recursive returns undefined when dir exists",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const result = mkdirSync(tempDir, { recursive: true });
    assertEquals(result, undefined);
    await Deno.remove(tempDir, { recursive: true });
  },
});

Deno.test({
  name: "[node/fs] promises.mkdir recursive returns first created directory",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const nested = path.join(tempDir, "a", "b", "c");
    const result = await mkdirPromise(nested, { recursive: true });
    assertEquals(result, path.join(tempDir, "a"));
    await Deno.remove(tempDir, { recursive: true });
  },
});

Deno.test({
  name: "[node/fs] promises.mkdir recursive returns undefined when dir exists",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const result = await mkdirPromise(tempDir, { recursive: true });
    assertEquals(result, undefined);
    await Deno.remove(tempDir, { recursive: true });
  },
});

Deno.test("[std/node/fs] mkdir callback isn't called twice if error is thrown", async () => {
  const tempDir = await Deno.makeTempDir();
  const subdir = path.join(tempDir, "subdir");
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { mkdir } from ${JSON.stringify(importUrl)}`,
    invocation: `mkdir(${JSON.stringify(subdir)}, `,
    async cleanup() {
      await Deno.remove(tempDir, { recursive: true });
    },
  });
});
