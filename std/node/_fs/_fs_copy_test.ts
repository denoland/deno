// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { copyFile, copyFileSync } from "./_fs_copy.ts";
import { existsSync } from "./_fs_exists.ts";

import { assert } from "../../testing/asserts.ts";
const { test } = Deno;

const destFile = "./destination.txt";

test({
  name: "[std/node/fs] copy file",
  fn: async () => {
    const srouceFile = Deno.makeTempFileSync();
    const err = await new Promise((resolve) => {
      copyFile(srouceFile, destFile, (err?: Error | null) => resolve(err));
    });
    assert(!err);
    assert(existsSync(destFile));
    Deno.removeSync(srouceFile);
    Deno.removeSync(destFile);
  },
});

test({
  name: "[std/node/fs] copy file sync",
  fn: () => {
    const srouceFile = Deno.makeTempFileSync();
    copyFileSync(srouceFile, destFile);
    assert(existsSync(destFile));
    Deno.removeSync(srouceFile);
    Deno.removeSync(destFile);
  },
});
