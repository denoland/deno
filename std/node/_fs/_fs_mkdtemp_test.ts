// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertThrows,
  assertThrowsAsync,
} from "../../testing/asserts.ts";
import { mkdtemp, mkdtempSync } from "./_fs_mkdtemp.ts";
import { existsSync } from "./_fs_exists.ts";
import { env } from "../process.ts";
import { isWindows } from "../../_util/os.ts";
import { promisify } from "../_util/_util_promisify.ts";

const prefix = isWindows ? env.TEMP + "\\" : (env.TMPDIR || "/tmp") + "/";
const doesNotExists = "/does/not/exists/";
const options = { encoding: "ascii" };
const badOptions = { encoding: "bogus" };

const mkdtempP = promisify(mkdtemp);

Deno.test({
  name: "[node/fs] mkdtemp",
  fn: async () => {
    const directory = await mkdtempP(prefix);
    assert(existsSync(directory));
    Deno.removeSync(directory);
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (does not exists)",
  fn: async () => {
    await assertThrowsAsync(() => mkdtempP(doesNotExists));
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (with options)",
  fn: async () => {
    const directory = await mkdtempP(prefix, options);
    assert(existsSync(directory));
    Deno.removeSync(directory);
  },
});

Deno.test({
  name: "[node/fs] mkdtemp (with bad options)",
  fn: async () => {
    await assertThrowsAsync(() => mkdtempP(prefix, badOptions));
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
    assertThrows(() => mkdtempSync(doesNotExists));
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
    assertThrows(() => mkdtempSync(prefix, badOptions));
  },
});
