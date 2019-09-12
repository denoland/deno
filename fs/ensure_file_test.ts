// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertThrows, assertThrowsAsync } from "../testing/asserts.ts";
import { ensureFile, ensureFileSync } from "./ensure_file.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function ensureFileIfItNotExist(): Promise<void> {
  const testDir = path.join(testdataDir, "ensure_file_1");
  const testFile = path.join(testDir, "test.txt");

  await ensureFile(testFile);

  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.stat(testFile).then((): void => {
        throw new Error("test file should exists.");
      });
    }
  );

  await Deno.remove(testDir, { recursive: true });
});

test(function ensureFileSyncIfItNotExist(): void {
  const testDir = path.join(testdataDir, "ensure_file_2");
  const testFile = path.join(testDir, "test.txt");

  ensureFileSync(testFile);

  assertThrows((): void => {
    Deno.statSync(testFile);
    throw new Error("test file should exists.");
  });

  Deno.removeSync(testDir, { recursive: true });
});

test(async function ensureFileIfItExist(): Promise<void> {
  const testDir = path.join(testdataDir, "ensure_file_3");
  const testFile = path.join(testDir, "test.txt");

  await Deno.mkdir(testDir, true);
  await Deno.writeFile(testFile, new Uint8Array());

  await ensureFile(testFile);

  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.stat(testFile).then((): void => {
        throw new Error("test file should exists.");
      });
    }
  );

  await Deno.remove(testDir, { recursive: true });
});

test(function ensureFileSyncIfItExist(): void {
  const testDir = path.join(testdataDir, "ensure_file_4");
  const testFile = path.join(testDir, "test.txt");

  Deno.mkdirSync(testDir, true);
  Deno.writeFileSync(testFile, new Uint8Array());

  ensureFileSync(testFile);

  assertThrows((): void => {
    Deno.statSync(testFile);
    throw new Error("test file should exists.");
  });

  Deno.removeSync(testDir, { recursive: true });
});

test(async function ensureFileIfItExistAsDir(): Promise<void> {
  const testDir = path.join(testdataDir, "ensure_file_5");

  await Deno.mkdir(testDir, true);

  await assertThrowsAsync(
    async (): Promise<void> => {
      await ensureFile(testDir);
    },
    Error,
    `Ensure path exists, expected 'file', got 'dir'`
  );

  await Deno.remove(testDir, { recursive: true });
});

test(function ensureFileSyncIfItExistAsDir(): void {
  const testDir = path.join(testdataDir, "ensure_file_6");

  Deno.mkdirSync(testDir, true);

  assertThrows(
    (): void => {
      ensureFileSync(testDir);
    },
    Error,
    `Ensure path exists, expected 'file', got 'dir'`
  );

  Deno.removeSync(testDir, { recursive: true });
});
