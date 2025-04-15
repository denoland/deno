// Copyright 2018-2025 the Deno authors. MIT license.
import * as path from "@std/path";
import { assert } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { existsSync, mkdir, mkdirSync } from "node:fs";

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
