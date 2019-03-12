// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertThrows, assertThrowsAsync } from "../testing/asserts.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function ensureDirIfItNotExist() {
  const baseDir = path.join(testdataDir, "ensure_dir_not_exist");
  const testDir = path.join(baseDir, "test");

  await ensureDir(testDir);

  assertThrowsAsync(async () => {
    await Deno.stat(testDir).then(() => {
      throw new Error("test dir should exists.");
    });
  });

  await Deno.remove(baseDir, { recursive: true });
});

test(function ensureDirSyncIfItNotExist() {
  const baseDir = path.join(testdataDir, "ensure_dir_sync_not_exist");
  const testDir = path.join(baseDir, "test");

  ensureDirSync(testDir);

  assertThrows(() => {
    Deno.statSync(testDir);
    throw new Error("test dir should exists.");
  });

  Deno.removeSync(baseDir, { recursive: true });
});

test(async function ensureDirIfItExist() {
  const baseDir = path.join(testdataDir, "ensure_dir_exist");
  const testDir = path.join(baseDir, "test");

  // create test directory
  await Deno.mkdir(testDir, true);

  await ensureDir(testDir);

  assertThrowsAsync(async () => {
    await Deno.stat(testDir).then(() => {
      throw new Error("test dir should still exists.");
    });
  });

  await Deno.remove(baseDir, { recursive: true });
});

test(function ensureDirSyncIfItExist() {
  const baseDir = path.join(testdataDir, "ensure_dir_sync_exist");
  const testDir = path.join(baseDir, "test");

  // create test directory
  Deno.mkdirSync(testDir, true);

  ensureDirSync(testDir);

  assertThrows(() => {
    Deno.statSync(testDir);
    throw new Error("test dir should still exists.");
  });

  Deno.removeSync(baseDir, { recursive: true });
});
