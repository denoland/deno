// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync
} from "../testing/asserts.ts";
import { emptyDir, emptyDirSync } from "./empty_dir.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function emptyDirIfItNotExist(): Promise<void> {
  const testDir = path.join(testdataDir, "empty_dir_test_1");
  const testNestDir = path.join(testDir, "nest");
  // empty a dir which not exist. then it will create new one
  await emptyDir(testNestDir);

  try {
    // check the dir
    const stat = await Deno.stat(testNestDir);
    assertEquals(stat.isDirectory(), true);
  } finally {
    // remove the test dir
    Deno.remove(testDir, { recursive: true });
  }
});

test(function emptyDirSyncIfItNotExist(): void {
  const testDir = path.join(testdataDir, "empty_dir_test_2");
  const testNestDir = path.join(testDir, "nest");
  // empty a dir which not exist. then it will create new one
  emptyDirSync(testNestDir);

  try {
    // check the dir
    const stat = Deno.statSync(testNestDir);
    assertEquals(stat.isDirectory(), true);
  } finally {
    // remove the test dir
    Deno.remove(testDir, { recursive: true });
  }
});

test(async function emptyDirIfItExist(): Promise<void> {
  const testDir = path.join(testdataDir, "empty_dir_test_3");
  const testNestDir = path.join(testDir, "nest");
  // create test dir
  await emptyDir(testNestDir);
  const testDirFile = path.join(testNestDir, "test.ts");
  // create test file in test dir
  await Deno.writeFile(testDirFile, new Uint8Array());

  // before empty: make sure file/directory exist
  const beforeFileStat = await Deno.stat(testDirFile);
  assertEquals(beforeFileStat.isFile(), true);

  const beforeDirStat = await Deno.stat(testNestDir);
  assertEquals(beforeDirStat.isDirectory(), true);

  await emptyDir(testDir);

  // after empty: file/directory have already remove
  try {
    // test dir still there
    const stat = await Deno.stat(testDir);
    assertEquals(stat.isDirectory(), true);

    // nest directory have been remove
    await assertThrowsAsync(
      async (): Promise<void> => {
        await Deno.stat(testNestDir);
      }
    );

    // test file have been remove
    await assertThrowsAsync(
      async (): Promise<void> => {
        await Deno.stat(testDirFile);
      }
    );
  } finally {
    // remote test dir
    await Deno.remove(testDir, { recursive: true });
  }
});

test(function emptyDirSyncIfItExist(): void {
  const testDir = path.join(testdataDir, "empty_dir_test_4");
  const testNestDir = path.join(testDir, "nest");
  // create test dir
  emptyDirSync(testNestDir);
  const testDirFile = path.join(testNestDir, "test.ts");
  // create test file in test dir
  Deno.writeFileSync(testDirFile, new Uint8Array());

  // before empty: make sure file/directory exist
  const beforeFileStat = Deno.statSync(testDirFile);
  assertEquals(beforeFileStat.isFile(), true);

  const beforeDirStat = Deno.statSync(testNestDir);
  assertEquals(beforeDirStat.isDirectory(), true);

  emptyDirSync(testDir);

  // after empty: file/directory have already remove
  try {
    // test dir still there
    const stat = Deno.statSync(testDir);
    assertEquals(stat.isDirectory(), true);

    // nest directory have been remove
    assertThrows((): void => {
      Deno.statSync(testNestDir);
    });

    // test file have been remove
    assertThrows((): void => {
      Deno.statSync(testDirFile);
    });
  } finally {
    // remote test dir
    Deno.removeSync(testDir, { recursive: true });
  }
});
