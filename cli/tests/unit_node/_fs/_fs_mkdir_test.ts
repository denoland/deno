// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import * as path from "../../../../test_util/std/path/mod.ts";
import { assert } from "../../../../test_util/std/assert/mod.ts";
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
