// Copyright 2018-2026 the Deno authors. MIT license.
import * as path from "@std/path";
import { assert, assertThrows } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { existsSync, mkdir, mkdirSync } from "node:fs";

const tmpDir = "./tmpdir";

function assertNodePermissionError(
  err: unknown,
  syscall: string,
  path?: string,
) {
  assert(err instanceof Error);
  assert(!(err instanceof Deno.errors.NotCapable));

  const nodeErr = err as NodeJS.ErrnoException;
  assert(nodeErr.code === "EPERM");
  assert(nodeErr.errno === -1);
  assert(nodeErr.syscall === syscall);
  if (path !== undefined) {
    assert(nodeErr.path === path);
  }
}

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
  name: "[node/fs] mkdirSync maps denied write permission to Node EPERM",
  permissions: { write: false },
  fn: () => {
    const dir = path.join(Deno.cwd(), "_fs_mkdirSync_denied_write");
    const err = assertThrows(() => mkdirSync(dir));
    assertNodePermissionError(err, "mkdir", dir);
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
