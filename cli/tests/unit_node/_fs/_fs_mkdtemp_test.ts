// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertRejects,
  assertThrows,
} from "../../../../test_util/std/assert/mod.ts";
import { EncodingOption, existsSync, mkdtemp, mkdtempSync } from "node:fs";
import { env } from "node:process";
import { promisify } from "node:util";

const prefix = Deno.build.os === "windows"
  ? env.TEMP + "\\"
  : (env.TMPDIR || "/tmp") + "/";
const doesNotExists = "/does/not/exists/";
const options: EncodingOption = { encoding: "ascii" };
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
    await assertRejects(() => mkdtempP(doesNotExists));
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
    // @ts-expect-error No overload matches this call
    await assertRejects(() => mkdtempP(prefix, badOptions));
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
    // @ts-expect-error No overload matches this call
    assertThrows(() => mkdtempSync(prefix, badOptions));
  },
});
