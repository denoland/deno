// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
import { mkdtemp } from "./_fs_mkdtemp.ts";
import { existsSync } from "./_fs_exists.ts";
import { env } from "../process.ts";
import { isWindows } from "../../_util/os.ts";

const prefix = isWindows ? env.TEMP : env.TMPDIR;
const doesNotExists = "/does/not/exists/";

Deno.test({
  name: "[node/fs] mkdir",
  fn: async () => {
    const result = await new Promise((resolve) => {
      mkdtemp(prefix, (err, directory) => {
        if (err) {
          resolve(false);
        } else if (directory) {
          resolve(existsSync(directory));
          Deno.removeSync(prefix);
        } else {
          resolve(false);
        }
      });
    });
    assert(result);
  },
});

Deno.test({
  name: "[node/fs] mkdir (does not exists)",
  fn: async () => {
    const result = await new Promise((resolve) => {
      mkdtemp(doesNotExists, (err, directory) => {
        if (err) {
          resolve(true);
        } else {
          resolve(false);
        }
      });
    });
    assert(result);
  },
});
