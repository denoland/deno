// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertRejects,
  assertStringIncludes,
  assertThrows,
} from "../assert/mod.ts";
import * as path from "../path/mod.ts";
import { emptyDir, emptyDirSync } from "./empty_dir.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = path.resolve(moduleDir, "testdata");

Deno.test("emptyDir() creates a new dir if it does not exist", async function () {
  const testDir = path.join(testdataDir, "empty_dir_test_1");
  const testNestDir = path.join(testDir, "nest");
  // empty a dir which not exist. then it will create new one
  await emptyDir(testNestDir);

  try {
    // check the dir
    const stat = await Deno.stat(testNestDir);
    assertEquals(stat.isDirectory, true);
  } finally {
    // remove the test dir
    await Deno.remove(testDir, { recursive: true });
  }
});

Deno.test("emptyDirSync() creates a new dir if it does not exist", function () {
  const testDir = path.join(testdataDir, "empty_dir_test_2");
  const testNestDir = path.join(testDir, "nest");
  // empty a dir which does not exist, then it will a create new one.
  emptyDirSync(testNestDir);

  try {
    // check the dir
    const stat = Deno.statSync(testNestDir);
    assertEquals(stat.isDirectory, true);
  } finally {
    // remove the test dir
    Deno.removeSync(testDir, { recursive: true });
  }
});

Deno.test("emptyDir() empties nested dirs and files", async function () {
  const testDir = path.join(testdataDir, "empty_dir_test_3");
  const testNestDir = path.join(testDir, "nest");
  // create test dir
  await emptyDir(testNestDir);
  const testDirFile = path.join(testNestDir, "test.ts");
  // create test file in test dir
  await Deno.writeFile(testDirFile, new Uint8Array());

  // before empty: make sure file/directory exist
  const beforeFileStat = await Deno.stat(testDirFile);
  assertEquals(beforeFileStat.isFile, true);

  const beforeDirStat = await Deno.stat(testNestDir);
  assertEquals(beforeDirStat.isDirectory, true);

  await emptyDir(testDir);

  // after empty: file/directory have already been removed
  try {
    // test dir still there
    const stat = await Deno.stat(testDir);
    assertEquals(stat.isDirectory, true);

    // nest directory have been removed
    await assertRejects(
      async () => {
        await Deno.stat(testNestDir);
      },
    );

    // test file have been removed
    await assertRejects(
      async () => {
        await Deno.stat(testDirFile);
      },
    );
  } finally {
    // remote test dir
    await Deno.remove(testDir, { recursive: true });
  }
});

Deno.test("emptyDirSync() empties nested dirs and files", function () {
  const testDir = path.join(testdataDir, "empty_dir_test_4");
  const testNestDir = path.join(testDir, "nest");
  // create test dir
  emptyDirSync(testNestDir);
  const testDirFile = path.join(testNestDir, "test.ts");
  // create test file in test dir
  Deno.writeFileSync(testDirFile, new Uint8Array());

  // before empty: make sure file/directory exist
  const beforeFileStat = Deno.statSync(testDirFile);
  assertEquals(beforeFileStat.isFile, true);

  const beforeDirStat = Deno.statSync(testNestDir);
  assertEquals(beforeDirStat.isDirectory, true);

  emptyDirSync(testDir);

  // after empty: file/directory have already remove
  try {
    // test dir still present
    const stat = Deno.statSync(testDir);
    assertEquals(stat.isDirectory, true);

    // nest directory have been removed
    assertThrows(() => {
      Deno.statSync(testNestDir);
    });

    // test file have been removed
    assertThrows(() => {
      Deno.statSync(testDirFile);
    });
  } finally {
    // remote test dir
    Deno.removeSync(testDir, { recursive: true });
  }
});

interface Scenes {
  read: boolean; // --allow-read
  write: boolean; // --allow-write
  async: boolean;
  output: string;
}
const scenes: Scenes[] = [
  // 1
  {
    read: false,
    write: false,
    async: true,
    output: "run again with the --allow-read flag",
  },
  {
    read: false,
    write: false,
    async: false,
    output: "run again with the --allow-read flag",
  },
  // 2
  {
    read: true,
    write: false,
    async: true,
    output: "run again with the --allow-write flag",
  },
  {
    read: true,
    write: false,
    async: false,
    output: "run again with the --allow-write flag",
  },
  // 3
  {
    read: false,
    write: true,
    async: true,
    output: "run again with the --allow-read flag",
  },
  {
    read: false,
    write: true,
    async: false,
    output: "run again with the --allow-read flag",
  },
  // 4
  {
    read: true,
    write: true,
    async: true,
    output: "success",
  },
  {
    read: true,
    write: true,
    async: false,
    output: "success",
  },
];
for (const s of scenes) {
  let title = `${s.async ? "emptyDir()" : "emptyDirSync()"}`;
  title += ` test ("testdata/testfolder") ${s.read ? "with" : "without"}`;
  title += ` --allow-read & ${s.write ? "with" : "without"} --allow-write`;
  Deno.test(`${title} permission`, async function (): Promise<
    void
  > {
    const testfolder = path.join(testdataDir, "testfolder");

    try {
      await Deno.mkdir(testfolder);

      await Deno.writeTextFile(
        path.join(testfolder, "child.txt"),
        "hello world",
      );

      try {
        const args = ["run", "--quiet", "--no-prompt"];

        if (s.read) {
          args.push("--allow-read");
        }

        if (s.write) {
          args.push("--allow-write");
        }

        args.push(
          path.join(
            testdataDir,
            s.async ? "empty_dir.ts" : "empty_dir_sync.ts",
          ),
        );
        args.push("testfolder");

        const command = new Deno.Command(Deno.execPath(), {
          cwd: testdataDir,
          args,
        });
        const { stdout } = await command.output();
        assertStringIncludes(new TextDecoder().decode(stdout), s.output);
      } catch (err) {
        await Deno.remove(testfolder, { recursive: true });
        throw err;
      }
    } finally {
      // Make the test rerunnable
      // Otherwise it would throw an error due to mkdir fail.
      await Deno.remove(testfolder, { recursive: true });
      // done
    }
  });
}
