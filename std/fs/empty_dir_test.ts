// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import {
  assertEquals,
  assertStrContains,
  assertThrows,
  assertThrowsAsync
} from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { emptyDir, emptyDirSync } from "./empty_dir.ts";

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

test(async function emptyDirPermission(): Promise<void> {
  interface Scenes {
    read: boolean; // --allow-read
    write: boolean; // --allow-write
    async: boolean;
    output: string;
  }

  const testfolder = path.join(testdataDir, "testfolder");

  await Deno.mkdir(testfolder);

  await Deno.writeFile(
    path.join(testfolder, "child.txt"),
    new TextEncoder().encode("hello world")
  );

  const scenes: Scenes[] = [
    // 1
    {
      read: false,
      write: false,
      async: true,
      output: "run again with the --allow-read flag"
    },
    {
      read: false,
      write: false,
      async: false,
      output: "run again with the --allow-read flag"
    },
    // 2
    {
      read: true,
      write: false,
      async: true,
      output: "run again with the --allow-write flag"
    },
    {
      read: true,
      write: false,
      async: false,
      output: "run again with the --allow-write flag"
    },
    // 3
    {
      read: false,
      write: true,
      async: true,
      output: "run again with the --allow-read flag"
    },
    {
      read: false,
      write: true,
      async: false,
      output: "run again with the --allow-read flag"
    },
    // 4
    {
      read: true,
      write: true,
      async: true,
      output: "success"
    },
    {
      read: true,
      write: true,
      async: false,
      output: "success"
    }
  ];

  try {
    for (const s of scenes) {
      console.log(
        `test ${s.async ? "emptyDir" : "emptyDirSync"}("testdata/testfolder") ${
          s.read ? "with" : "without"
        } --allow-read & ${s.write ? "with" : "without"} --allow-write`
      );

      const args = [Deno.execPath(), "run"];

      if (s.read) {
        args.push("--allow-read");
      }

      if (s.write) {
        args.push("--allow-write");
      }

      args.push(
        path.join(testdataDir, s.async ? "empty_dir.ts" : "empty_dir_sync.ts")
      );
      args.push("testfolder");

      const { stdout } = Deno.run({
        stdout: "piped",
        cwd: testdataDir,
        args: args
      });

      const output = await Deno.readAll(stdout);

      assertStrContains(new TextDecoder().decode(output), s.output);
    }
  } catch (err) {
    await Deno.remove(testfolder, { recursive: true });
    throw err;
  }
  // Make the test rerunnable
  // Otherwise would throw error due to mkdir fail.
  await Deno.remove(testfolder, { recursive: true });
  // done
});
