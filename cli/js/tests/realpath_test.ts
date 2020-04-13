// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

unitTest({ perms: { read: true } }, function realpathSyncSuccess(): void {
  const incompletePath = "cli/tests/fixture.json";
  const realPath = Deno.realpathSync(incompletePath);
  if (Deno.build.os !== "win") {
    assert(realPath.startsWith("/"));
  } else {
    assert(/^[A-Z]/.test(realPath));
  }
  assert(realPath.endsWith(incompletePath));
});

unitTest(
  {
    ignore: Deno.build.os === "win",
    perms: { read: true, write: true },
  },
  function realpathSyncSymlink(): void {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = Deno.realpathSync(symlink);
    assert(targetPath.startsWith("/"));
    assert(targetPath.endsWith("/target"));
  }
);

unitTest({ perms: { read: false } }, function realpathSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.realpathSync("some_file");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function realpathSyncNotFound(): void {
  let caughtError = false;
  try {
    Deno.realpathSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, async function realpathSuccess(): Promise<
  void
> {
  const incompletePath = "cli/tests/fixture.json";
  const realPath = await Deno.realpath(incompletePath);
  if (Deno.build.os !== "win") {
    assert(realPath.startsWith("/"));
  } else {
    assert(/^[A-Z]/.test(realPath));
  }
  assert(realPath.endsWith(incompletePath));
});

unitTest(
  {
    ignore: Deno.build.os === "win",
    perms: { read: true, write: true },
  },
  async function realpathSymlink(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = await Deno.realpath(symlink);
    assert(targetPath.startsWith("/"));
    assert(targetPath.endsWith("/target"));
  }
);

unitTest({ perms: { read: false } }, async function realpathPerm(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.realpath("some_file");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, async function realpathNotFound(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.realpath("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
});
