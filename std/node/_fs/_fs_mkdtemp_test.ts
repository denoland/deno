// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
import { mkdtemp, mkdtempSync } from "./_fs_mkdtemp.ts";
import { existsSync } from "./_fs_exists.ts";
import { env } from "../process.ts";
import { isWindows } from "../../_util/os.ts";
import { promisify } from "../_util/_util_promisify.ts";

const prefix = isWindows ? env.TEMP + "\\" : (env.TMPDIR || "/tmp") + "/";
const doesNotExists = "/does/not/exists/";
const options = { encoding: "ascii" };
const badOptions = { encoding: "bogus" };

const mkdtempP = promisify(mkdtemp)

Deno.test({
  name: "[node/fs] mkdtemp",
  fn: async () => {
    try {
      const directory = await mkdtempP(prefix);
      assert(existsSync(directory));
      Deno.removeSync(directory);
    } catch (error) {
      assert(false);
    }
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (does not exists)",
  fn: async () => {
    try {
      const directory = await mkdtempP(doesNotExists);
      
      // should have thrown already...
      assert(!existsSync(directory));
      Deno.removeSync(directory);

    } catch (error) {
      assert(true);
    }
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (with options)",
  fn: async () => {
    try {
      const directory = await mkdtempP(prefix, options);
      assert(existsSync(directory));
      Deno.removeSync(directory);

    } catch (error) {
      assert(false);
    }
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (with bad options)",
  fn: async () => {
    try {
      const directory = await mkdtempP(prefix, badOptions);
      
      // should have thrown already...
      assert(!existsSync(directory));
      Deno.removeSync(directory);

    } catch (error) {
      assert(true);
    }
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
