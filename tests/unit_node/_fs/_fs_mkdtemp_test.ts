// Copyright 2018-2025 the Deno authors. MIT license.
import { assert, assertRejects, assertThrows, fail } from "@std/assert";
import { Buffer } from "node:buffer";
import { EncodingOption, existsSync, mkdtemp, mkdtempSync } from "node:fs";
import { env } from "node:process";
import { promisify } from "node:util";
import { join } from "@std/path";

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
  name: "[node/fs] mkdtemp (buffer encoding)",
  fn: async () => {
    const dirBuffer = await mkdtempP(prefix, { encoding: "buffer" });
    assert(dirBuffer instanceof Buffer);
    const directory = dirBuffer.toString();
    assert(existsSync(directory));
    Deno.removeSync(directory);
  },
});

Deno.test({
  name: "[node/fs] mkdtemp assert error",
  fn: async () => {
    const recursiveDir = join("noop", prefix);
    try {
      await mkdtempP(recursiveDir);
      fail("mkdtemp should have failed");
    } catch (err) {
      // deno-lint-ignore no-explicit-any
      assert((err as any).code === "ENOENT");
      // deno-lint-ignore no-explicit-any
      assert((err as any).syscall === "mkdtemp");
      // deno-lint-ignore no-explicit-any
      assert((err as any).path === recursiveDir + "XXXXXX");
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

Deno.test({
  name: "[node/fs] mkdtempSync (buffer encoding)",
  fn: () => {
    const dirBuffer = mkdtempSync(prefix, { encoding: "buffer" });
    assert(dirBuffer instanceof Buffer);
    const directory = dirBuffer.toString();
    assert(existsSync(directory));
    Deno.removeSync(directory);
  },
});

Deno.test({
  name: "[node/fs] mkdtempSync assert error",
  fn: () => {
    const recursiveDir = join("noop", prefix);
    try {
      mkdtempSync(recursiveDir);
      fail("mkdtemp should have failed");
    } catch (err) {
      // deno-lint-ignore no-explicit-any
      assert((err as any).code === "ENOENT");
      // deno-lint-ignore no-explicit-any
      assert((err as any).syscall === "mkdtemp");
      // deno-lint-ignore no-explicit-any
      assert((err as any).path === recursiveDir + "XXXXXX");
    }
  },
});
