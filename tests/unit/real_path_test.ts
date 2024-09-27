// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertMatch,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

Deno.test({ permissions: { read: true } }, function realPathSyncSuccess() {
  const relative = "tests/testdata/assets/fixture.json";
  const realPath = Deno.realPathSync(relative);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
    assert(realPath.endsWith(relative));
  } else {
    assertMatch(realPath, /^[A-Z]:\\/);
    assert(realPath.endsWith(relative.replace(/\//g, "\\")));
  }
});

Deno.test({ permissions: { read: true } }, function realPathSyncUrl() {
  const relative = "tests/testdata/assets/fixture.json";
  const url = pathToAbsoluteFileUrl(relative);
  assertEquals(Deno.realPathSync(relative), Deno.realPathSync(url));
});

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  function realPathSyncSymlink() {
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
  },
);

Deno.test({ permissions: { read: false } }, function realPathSyncPerm() {
  assertThrows(() => {
    Deno.realPathSync("some_file");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, function realPathSyncNotFound() {
  assertThrows(() => {
    Deno.realPathSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test({ permissions: { read: true } }, async function realPathSuccess() {
  const relativePath = "tests/testdata/assets/fixture.json";
  const realPath = await Deno.realPath(relativePath);
  if (Deno.build.os !== "windows") {
    assert(realPath.startsWith("/"));
    assert(realPath.endsWith(relativePath));
  } else {
    assertMatch(realPath, /^[A-Z]:\\/);
    assert(realPath.endsWith(relativePath.replace(/\//g, "\\")));
  }
});

Deno.test(
  { permissions: { read: true } },
  async function realPathUrl() {
    const relative = "tests/testdata/assets/fixture.json";
    const url = pathToAbsoluteFileUrl(relative);
    assertEquals(await Deno.realPath(relative), await Deno.realPath(url));
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  async function realPathSymlink() {
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
  },
);

Deno.test({ permissions: { read: false } }, async function realPathPerm() {
  await assertRejects(async () => {
    await Deno.realPath("some_file");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, async function realPathNotFound() {
  await assertRejects(async () => {
    await Deno.realPath("bad_filename");
  }, Deno.errors.NotFound);
});
