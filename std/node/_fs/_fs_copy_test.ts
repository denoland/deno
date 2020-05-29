// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { copyFile, copyFileSync } from "./_fs_copy.ts";
import { existsSync } from "./_fs_exists.ts";

import { assert } from "../../testing/asserts.ts";
const { test } = Deno;

const destFile = "./destination.txt";

test({
  name: "[std/node/fs] copy file",
  fn: async () => {
    const sourceFile = Deno.makeTempFileSync();
    const err = await new Promise((resolve) => {
      copyFile(sourceFile, destFile, (err?: Error | null) => resolve(err));
    });
    assert(!err);
    assert(existsSync(destFile));
    Deno.removeSync(sourceFile);
    Deno.removeSync(destFile);
  },
});

test({
  name: "[std/node/fs] copy file sync",
  fn: () => {
    const sourceFile = Deno.makeTempFileSync();
    copyFileSync(sourceFile, destFile);
    assert(existsSync(destFile));
    Deno.removeSync(sourceFile);
    Deno.removeSync(destFile);
  },
});
