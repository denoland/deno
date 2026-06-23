// Copyright 2018-2026 the Deno authors. MIT license.
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

Deno.test({
  name: "[node/fs] mkdirSync recursive path with dot segment",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    try {
      const normalizedTarget = path.join(tempDir, "a", "b", "c");
      const target = normalizedTarget + path.SEPARATOR + "." + path.SEPARATOR;
      assert(target.endsWith(`${path.SEPARATOR}.${path.SEPARATOR}`));
      const firstCreated = mkdirSync(target, { recursive: true });

      assert(existsSync(normalizedTarget));
      assert(firstCreated);
      assert(existsSync(firstCreated));
    } finally {
      await Deno.remove(tempDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "[node/fs] mkdirSync recursive path preserves parent segment",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    try {
      const lexicalParent = path.join(tempDir, "a");
      const target = lexicalParent + path.SEPARATOR + ".." +
        path.SEPARATOR + "b" + path.SEPARATOR + "c" + path.SEPARATOR + ".";
      mkdirSync(target, { recursive: true });

      assert(existsSync(target));
      if (Deno.build.os !== "windows") {
        assert(existsSync(lexicalParent));
      }
    } finally {
      await Deno.remove(tempDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "[node/fs] mkdir recursive callback path with dot segment",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    try {
      const normalizedTarget = path.join(tempDir, "a", "b", "c");
      const target = normalizedTarget + path.SEPARATOR + "." + path.SEPARATOR;
      assert(target.endsWith(`${path.SEPARATOR}.${path.SEPARATOR}`));
      const firstCreated = await new Promise<string | undefined>(
        (resolve, reject) => {
          mkdir(target, { recursive: true }, (err, path) => {
            if (err) {
              reject(err);
              return;
            }
            resolve(path);
          });
        },
      );

      assert(existsSync(normalizedTarget));
      assert(firstCreated);
      assert(existsSync(firstCreated));
    } finally {
      await Deno.remove(tempDir, { recursive: true });
    }
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
