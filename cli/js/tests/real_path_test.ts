// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

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
    ignore: Deno.build.os === "windows",
    perms: { read: true, write: true },
  },
  function realPathSyncSymlink(): void {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = Deno.realPathSync(symlink);
    assert(targetPath.startsWith("/"));
    assert(targetPath.endsWith("/target"));
  }
);

unitTest({ perms: { read: false } }, function realPathSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.realPathSync("some_file");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function realPathSyncNotFound(): void {
  let caughtError = false;
  try {
    Deno.realPathSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
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
    ignore: Deno.build.os === "windows",
    perms: { read: true, write: true },
  },
  async function realPathSymlink(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = await Deno.realPath(symlink);
    assert(targetPath.startsWith("/"));
    assert(targetPath.endsWith("/target"));
  }
);

unitTest({ perms: { read: false } }, async function realPathPerm(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.realPath("some_file");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, async function realPathNotFound(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.realPath("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
});
