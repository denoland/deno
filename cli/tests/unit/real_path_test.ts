// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertMatch,
  assertThrows,
  assertThrowsAsync,
} from "./test_util.ts";

Deno.test("realPathSyncSuccess", function (): void {
  const relative = "cli/tests/fixture.json";
  const realPath = Deno.realPathSync(relative);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
    assert(realPath.endsWith(relative));
  } else {
    assertMatch(realPath, /^[A-Z]:\\/);
    assert(realPath.endsWith(relative.replace(/\//g, "\\")));
  }
});

Deno.test("realPathSyncSymlink", function (): void {
  const testDir = Deno.makeTempDirSync();
  const target = testDir + "/target";
  const symlink = testDir + "/symln";
  Deno.mkdirSync(target);
  Deno.symlinkSync(target, symlink);
  const realPath = Deno.realPathSync(symlink);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
    assert(realPath.endsWith("/target"));
  } else {
    assertMatch(realPath, /^[A-Z]:\\/);
    assert(realPath.endsWith("\\target"));
  }
});

Deno.test("realPathSyncNotFound", function (): void {
  assertThrows(() => {
    Deno.realPathSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test("realPathSuccess", async function (): Promise<
  void
> {
  const relativePath = "cli/tests/fixture.json";
  const realPath = await Deno.realPath(relativePath);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
    assert(realPath.endsWith(relativePath));
  } else {
    assertMatch(realPath, /^[A-Z]:\\/);
    assert(realPath.endsWith(relativePath.replace(/\//g, "\\")));
  }
});

Deno.test("realPathSymlink", async function (): Promise<void> {
  const testDir = Deno.makeTempDirSync();
  const target = testDir + "/target";
  const symlink = testDir + "/symln";
  Deno.mkdirSync(target);
  Deno.symlinkSync(target, symlink);
  const realPath = await Deno.realPath(symlink);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
    assert(realPath.endsWith("/target"));
  } else {
    assertMatch(realPath, /^[A-Z]:\\/);
    assert(realPath.endsWith("\\target"));
  }
});

Deno.test("realPathNotFound", async function (): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.realPath("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test("realPathSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(() => {
    Deno.realPathSync("some_file");
  }, Deno.errors.PermissionDenied);
});

Deno.test("realPathPerm", async function (): Promise<
  void
> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.realPath("some_file");
  }, Deno.errors.PermissionDenied);
});
