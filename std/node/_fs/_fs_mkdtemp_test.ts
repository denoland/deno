// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
import { mkdtemp, mkdtempSync } from "./_fs_mkdtemp.ts";
import { existsSync } from "./_fs_exists.ts";
import { env } from "../process.ts";
import { isWindows } from "../../_util/os.ts";

const prefix = isWindows ? env.TEMP + "\\" : (env.TMPDIR || "/tmp") + "/";
const doesNotExists = "/does/not/exists/";
const options = { encoding: "ascii" };
const badOptions = { encoding: "bogus" };

Deno.test({
  name: "[node/fs] mkdtemp",
  fn: async () => {
    const result = await new Promise((resolve) => {
      mkdtemp(prefix, (err, directory) => {
        if (err) {
          resolve(false);
        } else if (directory) {
          resolve(existsSync(directory));
          Deno.removeSync(directory);
        } else {
          resolve(false);
        }
      });
    });
    assert(result);
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (does not exists)",
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

Deno.test({
  name: "[node/fs] mkdtemp (with options)",
  fn: async () => {
    const result = await new Promise((resolve) => {
      mkdtemp(prefix, options, (err, directory) => {
        if (err) {
          resolve(false);
        } else if (directory) {
          resolve(existsSync(directory));
          Deno.removeSync(directory);
        } else {
          resolve(false);
        }
      });
    });
    assert(result);
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (with bad options)",
  fn: async () => {
    const result = await new Promise((resolve) => {
      try {
        mkdtemp(prefix, badOptions, (err, directory) => {
          // should have thrown already...
          resolve(false);
        });
      } catch (error) {
        resolve(true);
      }
    });
    assert(result);
  },
});

Deno.test({
  name: "[node/fs] mkdtempSync",
  fn: () => {
    const directory = mkdtempSync(prefix);
    const dirExists = existsSync(directory);
    Deno.removeSync(directory);
    assert(dirExists);
  },
});

Deno.test({
  name: "[node/fs] mkdtempSync (does not exists)",
  fn: () => {
    try {
      const directory = mkdtempSync(doesNotExists);
      // should have thrown already...

      const dirExists = existsSync(directory);
      Deno.removeSync(directory);
      assert(!dirExists);
    } catch (error) {
      assert(true);
    }
  },
});

Deno.test({
  name: "[node/fs] mkdtempSync (with options)",
  fn: () => {
    const directory = mkdtempSync(prefix, options);
    const dirExists = existsSync(directory);
    Deno.removeSync(directory);
    assert(dirExists);
  },
});

Deno.test({
  name: "[node/fs] mkdtempSync (with bad options)",
  fn: () => {
    try {
      const directory = mkdtempSync(prefix, badOptions);
      // should have thrown already...

      Deno.removeSync(directory);
      assert(false);
    } catch (error) {
      assert(true);
    }
  },
});
