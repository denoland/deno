// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
import { existsSync } from "./_fs_exists.ts";
import { mkdir, mkdirSync } from "./_fs_mkdir.ts";

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
