// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertThrows,
  assertThrowsAsync,
} from "./test_util.ts";

unitTest({ perms: { read: true } }, function realPathSyncSuccess(): void {
  const incompletePath = "cli/tests/fixture.json";
  const realPath = Deno.realPathSync(incompletePath);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
  } else {
    assert(/^[A-Z]/.test(realPath));
  }
  assert(realPath.endsWith(incompletePath));
});

unitTest(
  {
    perms: { read: true, write: true },
  },
  function realPathSyncSymlink(): void {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = Deno.realPathSync(symlink);
    if (Deno.build.os !== "windows") {
      assert(targetPath.startsWith("/"));
    } else {
      assert(/^[A-Z]/.test(targetPath));
    }
    assert(targetPath.endsWith("/target"));
  },
);

unitTest({ perms: { read: false } }, function realPathSyncPerm(): void {
  assertThrows(() => {
    Deno.realPathSync("some_file");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function realPathSyncNotFound(): void {
  assertThrows(() => {
    Deno.realPathSync("bad_filename");
  }, Deno.errors.NotFound);
});

unitTest({ perms: { read: true } }, async function realPathSuccess(): Promise<
  void
> {
  const incompletePath = "cli/tests/fixture.json";
  const realPath = await Deno.realPath(incompletePath);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
  } else {
    assert(/^[A-Z]/.test(realPath));
  }
  assert(realPath.endsWith(incompletePath));
});

unitTest(
  {
    perms: { read: true, write: true },
  },
  async function realPathSymlink(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = await Deno.realPath(symlink);
    if (Deno.build.os !== "windows") {
      assert(targetPath.startsWith("/"));
    } else {
      assert(/^[A-Z]/.test(targetPath));
    }
    assert(targetPath.endsWith("/target"));
  },
);

unitTest({ perms: { read: false } }, async function realPathPerm(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.realPath("some_file");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, async function realPathNotFound(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.realPath("bad_filename");
  }, Deno.errors.NotFound);
});
