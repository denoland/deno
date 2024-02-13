// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { walk, WalkError, WalkOptions, walkSync } from "./walk.ts";
import {
  assertArrayIncludes,
  assertEquals,
  assertRejects,
  assertThrows,
} from "../assert/mod.ts";
import { fromFileUrl, resolve } from "../path/mod.ts";

const testdataDir = resolve(fromFileUrl(import.meta.url), "../testdata/walk");

async function assertWalkPaths(
  rootPath: string,
  expectedPaths: string[],
  options?: WalkOptions,
) {
  const root = resolve(testdataDir, rootPath);
  const entries = await Array.fromAsync(walk(root, options));

  const expected = expectedPaths.map((path) => resolve(root, path));
  assertEquals(entries.length, expected.length);
  assertArrayIncludes(entries.map(({ path }) => path), expected);
}

function assertWalkSyncPaths(
  rootPath: string,
  expectedPaths: string[],
  options?: WalkOptions,
) {
  const root = resolve(testdataDir, rootPath);
  const entriesSync = Array.from(walkSync(root, options));

  const expected = expectedPaths.map((path) => resolve(root, path));
  assertEquals(entriesSync.length, expected.length);
  assertArrayIncludes(entriesSync.map(({ path }) => path), expected);
}

Deno.test("walk() returns current dir for empty dir", async () => {
  const emptyDir = resolve(testdataDir, "empty_dir");
  await Deno.mkdir(emptyDir);
  await assertWalkPaths("empty_dir", ["."]);
  await Deno.remove(emptyDir);
});

Deno.test("walkSync() returns current dir for empty dir", async () => {
  const emptyDir = resolve(testdataDir, "empty_dir");
  await Deno.mkdir(emptyDir);
  assertWalkSyncPaths("empty_dir", ["."]);
  await Deno.remove(emptyDir);
});

Deno.test("walk() returns current dir and single file", async () =>
  await assertWalkPaths("single_file", [".", "x"]));

Deno.test("walkSync() returns current dir and single file", () =>
  assertWalkSyncPaths("single_file", [".", "x"]));

Deno.test("walk() returns current dir, subdir, and nested file", async () =>
  await assertWalkPaths("nested_single_file", [".", "a", "a/x"]));

Deno.test("walkSync() returns current dir, subdir, and nested file", () =>
  assertWalkSyncPaths("nested_single_file", [".", "a", "a/x"]));

Deno.test("walk() accepts maxDepth option", async () =>
  await assertWalkPaths("depth", [".", "a", "a/b", "a/b/c"], { maxDepth: 3 }));

Deno.test("walkSync() accepts maxDepth option", () =>
  assertWalkSyncPaths("depth", [".", "a", "a/b", "a/b/c"], { maxDepth: 3 }));

Deno.test("walk() accepts includeDirs option set to false", async () =>
  await assertWalkPaths("depth", ["a/b/c/d/x"], { includeDirs: false }));

Deno.test("walkSync() accepts includeDirs option set to false", () =>
  assertWalkSyncPaths("depth", ["a/b/c/d/x"], { includeDirs: false }));

Deno.test("walk() accepts includeFiles option set to false", async () =>
  await assertWalkPaths("depth", [".", "a", "a/b", "a/b/c", "a/b/c/d"], {
    includeFiles: false,
  }));

Deno.test("walkSync() accepts includeFiles option set to false", () =>
  assertWalkSyncPaths("depth", [".", "a", "a/b", "a/b/c", "a/b/c/d"], {
    includeFiles: false,
  }));

Deno.test("walk() accepts ext option as strings", async () =>
  await assertWalkPaths("ext", ["y.rs", "x.ts"], {
    exts: [".rs", ".ts"],
  }));

Deno.test("walkSync() accepts ext option as strings", () =>
  assertWalkSyncPaths("ext", ["y.rs", "x.ts"], {
    exts: [".rs", ".ts"],
  }));

Deno.test("walk() accepts ext option as regExps", async () =>
  await assertWalkPaths("match", ["x", "y"], {
    match: [/x/, /y/],
  }));

Deno.test("walkSync() accepts ext option as regExps", () =>
  assertWalkSyncPaths("match", ["x", "y"], {
    match: [/x/, /y/],
  }));

Deno.test("walk() accepts skip option as regExps", async () =>
  await assertWalkPaths("match", [".", "z"], {
    skip: [/x/, /y/],
  }));

Deno.test("walkSync() accepts skip option as regExps", () =>
  assertWalkSyncPaths("match", [".", "z"], {
    skip: [/x/, /y/],
  }));

// https://github.com/denoland/deno_std/issues/1358
Deno.test("walk() accepts followSymlinks option set to true", async () =>
  await assertWalkPaths("symlink", [".", "a", "a/z", "a", "a/z", "x", "x"], {
    followSymlinks: true,
  }));

Deno.test("walkSync() accepts followSymlinks option set to true", () =>
  assertWalkSyncPaths("symlink", [".", "a", "a/z", "a", "a/z", "x", "x"], {
    followSymlinks: true,
  }));

Deno.test("walk() accepts followSymlinks option set to true with canonicalize option set to false", async () =>
  await assertWalkPaths("symlink", [".", "a", "a/z", "b", "b/z", "x", "y"], {
    followSymlinks: true,
    canonicalize: false,
  }));

Deno.test("walkSync() accepts followSymlinks option set to true with canonicalize option set to false", () =>
  assertWalkSyncPaths("symlink", [".", "a", "a/z", "b", "b/z", "x", "y"], {
    followSymlinks: true,
    canonicalize: false,
  }));

Deno.test("walk() accepts followSymlinks option set to false", async () => {
  await assertWalkPaths("symlink", [".", "a", "a/z", "b", "x", "y"], {
    followSymlinks: false,
  });
});

Deno.test("walkSync() accepts followSymlinks option set to false", () => {
  assertWalkSyncPaths("symlink", [".", "a", "a/z", "b", "x", "y"], {
    followSymlinks: false,
  });
});

Deno.test("walk() rejects Deno.errors.NotFound for non-existent root", async () => {
  const root = resolve(testdataDir, "non_existent");
  await assertRejects(
    async () => await Array.fromAsync(walk(root)),
    Deno.errors.NotFound,
  );
});

Deno.test("walkSync() throws Deno.errors.NotFound for non-existent root", () => {
  const root = resolve(testdataDir, "non_existent");
  assertThrows(() => Array.from(walkSync(root)), Deno.errors.NotFound);
});

// https://github.com/denoland/deno_std/issues/1789
Deno.test({
  name: "walk() walks unix socket",
  ignore: Deno.build.os === "windows",
  async fn() {
    const path = resolve(testdataDir, "socket", "a.sock");
    try {
      const listener = Deno.listen({ path, transport: "unix" });
      await assertWalkPaths("socket", [".", "a.sock", ".gitignore"], {
        followSymlinks: true,
      });
      listener.close();
    } finally {
      await Deno.remove(path);
    }
  },
});

// https://github.com/denoland/deno_std/issues/1789
Deno.test({
  name: "walkSync() walks unix socket",
  ignore: Deno.build.os === "windows",
  async fn() {
    const path = resolve(testdataDir, "socket", "a.sock");
    try {
      const listener = Deno.listen({ path, transport: "unix" });
      assertWalkSyncPaths("socket", [".", "a.sock", ".gitignore"], {
        followSymlinks: true,
      });
      listener.close();
    } finally {
      await Deno.remove(path);
    }
  },
});

Deno.test({
  name: "walk() walks fifo files on unix",
  ignore: Deno.build.os === "windows",
  async fn() {
    const command = new Deno.Command("mkfifo", {
      args: [resolve(testdataDir, "fifo", "fifo")],
    });
    await command.output();
    await assertWalkPaths("fifo", [".", "fifo", ".gitignore"], {
      followSymlinks: true,
    });
  },
});

Deno.test({
  name: "walkSync() walks fifo files on unix",
  ignore: Deno.build.os === "windows",
  async fn() {
    const command = new Deno.Command("mkfifo", {
      args: [resolve(testdataDir, "fifo", "fifo")],
    });
    await command.output();
    assertWalkSyncPaths("fifo", [".", "fifo", ".gitignore"], {
      followSymlinks: true,
    });
  },
});

Deno.test("walk() rejects with WalkError when root is removed during execution", async () => {
  const root = resolve(testdataDir, "error");
  await Deno.mkdir(root);
  try {
    await assertRejects(async () => {
      await Array.fromAsync(
        walk(root),
        async () => await Deno.remove(root, { recursive: true }),
      );
    }, WalkError);
  } catch (err) {
    await Deno.remove(root, { recursive: true });
    throw err;
  }
});
