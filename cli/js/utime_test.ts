// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

// Allow 10 second difference.
// Note this might not be enough for FAT (but we are not testing on such fs).
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function assertFuzzyTimestampEquals(t1: any, t2: number): void {
  assert(typeof t1 === "number");
  assert(Math.abs(t1 - t2) < 10);
}

testPerm({ read: true, write: true }, function utimeSyncFileSuccess(): void {
  const testDir = Deno.makeTempDirSync();
  const filename = testDir + "/file.txt";
  Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
    perm: 0o666
  });

  const atime = 1000;
  const mtime = 50000;
  Deno.utimeSync(filename, atime, mtime);

  const fileInfo = Deno.statSync(filename);
  assertFuzzyTimestampEquals(fileInfo.accessed, atime);
  assertFuzzyTimestampEquals(fileInfo.modified, mtime);
});

testPerm(
  { read: true, write: true },
  function utimeSyncDirectorySuccess(): void {
    const testDir = Deno.makeTempDirSync();

    const atime = 1000;
    const mtime = 50000;
    Deno.utimeSync(testDir, atime, mtime);

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.accessed, atime);
    assertFuzzyTimestampEquals(dirInfo.modified, mtime);
  }
);

testPerm({ read: true, write: true }, function utimeSyncDateSuccess(): void {
  const testDir = Deno.makeTempDirSync();

  const atime = 1000;
  const mtime = 50000;
  Deno.utimeSync(testDir, new Date(atime * 1000), new Date(mtime * 1000));

  const dirInfo = Deno.statSync(testDir);
  assertFuzzyTimestampEquals(dirInfo.accessed, atime);
  assertFuzzyTimestampEquals(dirInfo.modified, mtime);
});

testPerm(
  { read: true, write: true },
  function utimeSyncLargeNumberSuccess(): void {
    const testDir = Deno.makeTempDirSync();

    // There are Rust side caps (might be fs relate),
    // so JUST make them slightly larger than UINT32_MAX.
    const atime = 0x100000001;
    const mtime = 0x100000002;
    Deno.utimeSync(testDir, atime, mtime);

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.accessed, atime);
    assertFuzzyTimestampEquals(dirInfo.modified, mtime);
  }
);

testPerm({ read: true, write: true }, function utimeSyncNotFound(): void {
  const atime = 1000;
  const mtime = 50000;

  let caughtError = false;
  try {
    Deno.utimeSync("/baddir", atime, mtime);
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
});

testPerm({ read: true, write: false }, function utimeSyncPerm(): void {
  const atime = 1000;
  const mtime = 50000;

  let caughtError = false;
  try {
    Deno.utimeSync("/some_dir", atime, mtime);
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

testPerm(
  { read: true, write: true },
  async function utimeFileSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
      perm: 0o666
    });

    const atime = 1000;
    const mtime = 50000;
    await Deno.utime(filename, atime, mtime);

    const fileInfo = Deno.statSync(filename);
    assertFuzzyTimestampEquals(fileInfo.accessed, atime);
    assertFuzzyTimestampEquals(fileInfo.modified, mtime);
  }
);

testPerm(
  { read: true, write: true },
  async function utimeDirectorySuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();

    const atime = 1000;
    const mtime = 50000;
    await Deno.utime(testDir, atime, mtime);

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.accessed, atime);
    assertFuzzyTimestampEquals(dirInfo.modified, mtime);
  }
);

testPerm(
  { read: true, write: true },
  async function utimeDateSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();

    const atime = 1000;
    const mtime = 50000;
    await Deno.utime(testDir, new Date(atime * 1000), new Date(mtime * 1000));

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.accessed, atime);
    assertFuzzyTimestampEquals(dirInfo.modified, mtime);
  }
);

testPerm({ read: true, write: true }, async function utimeNotFound(): Promise<
  void
> {
  const atime = 1000;
  const mtime = 50000;

  let caughtError = false;
  try {
    await Deno.utime("/baddir", atime, mtime);
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
});

testPerm({ read: true, write: false }, async function utimeSyncPerm(): Promise<
  void
> {
  const atime = 1000;
  const mtime = 50000;

  let caughtError = false;
  try {
    await Deno.utime("/some_dir", atime, mtime);
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

const isNotWindows = Deno.build.os !== "win";

if (isNotWindows) {
  testPerm({ read: true, write: true }, function futimeSyncFileSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
      perm: 0o666
    });

    const atime = 1000;
    const mtime = 50000;
    const f = Deno.openSync(filename, "r+");
    f.utimeSync(atime, mtime);
    f.close();

    const fileInfo = Deno.statSync(filename);
    assertFuzzyTimestampEquals(fileInfo.accessed, atime);
    assertFuzzyTimestampEquals(fileInfo.modified, mtime);
  });

  testPerm({ read: true, write: true }, function futimeSyncDateSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
      perm: 0o666
    });

    const atime = 1000;
    const mtime = 50000;
    const f = Deno.openSync(filename, "r+");
    f.utimeSync(new Date(atime * 1000), new Date(mtime * 1000));
    f.close();

    const fileInfo = Deno.statSync(filename);
    assertFuzzyTimestampEquals(fileInfo.accessed, atime);
    assertFuzzyTimestampEquals(fileInfo.modified, mtime);
  });

  testPerm(
    { read: true, write: true },
    function futimeSyncLargeNumberSuccess(): void {
      const testDir = Deno.makeTempDirSync();
      const filename = testDir + "/file.txt";
      Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
        perm: 0o666
      });

      // There are Rust side caps (might be fs relate),
      // so JUST make them slightly larger than UINT32_MAX.
      const atime = 0x100000001;
      const mtime = 0x100000002;
      const f = Deno.openSync(filename, "r+");
      f.utimeSync(atime, mtime);
      f.close();

      const fileInfo = Deno.statSync(filename);
      assertFuzzyTimestampEquals(fileInfo.accessed, atime);
      assertFuzzyTimestampEquals(fileInfo.modified, mtime);
    }
  );

  testPerm(
    { read: true, write: true },
    async function futimeFileSuccess(): Promise<void> {
      const testDir = Deno.makeTempDirSync();
      const filename = testDir + "/file.txt";
      Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
        perm: 0o666
      });

      const atime = 1000;
      const mtime = 50000;
      const f = await Deno.open(filename, "r+");
      await f.utime(atime, mtime);
      f.close();

      const fileInfo = Deno.statSync(filename);
      assertFuzzyTimestampEquals(fileInfo.accessed, atime);
      assertFuzzyTimestampEquals(fileInfo.modified, mtime);
    }
  );

  testPerm(
    { read: true, write: true },
    async function futimeDateSuccess(): Promise<void> {
      const testDir = Deno.makeTempDirSync();
      const filename = testDir + "/file.txt";
      Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
        perm: 0o666
      });

      const atime = 1000;
      const mtime = 50000;
      const f = await Deno.open(filename, "r+");
      await f.utime(new Date(atime * 1000), new Date(mtime * 1000));
      f.close();

      const fileInfo = Deno.statSync(filename);
      assertFuzzyTimestampEquals(fileInfo.accessed, atime);
      assertFuzzyTimestampEquals(fileInfo.modified, mtime);
    }
  );

  testPerm({ read: true, write: false }, function futimeSyncPerm(): void {
    let err;
    let caughtError = false;
    const atime = 1000;
    const mtime = 50000;
    const f = Deno.openSync("README.md", "r");
    try {
      f.utimeSync(atime, mtime);
    } catch (e) {
      caughtError = true;
      err = e;
    }
    f.close();
    // throw if we lack --write permissions
    assert(caughtError);
    if (caughtError) {
      assert(err instanceof Deno.errors.PermissionDenied);
      assertEquals(err.name, "PermissionDenied");
    }
  });

  testPerm({ read: true, write: false }, async function futimePerm(): Promise<
    void
  > {
    let err;
    let caughtError = false;
    const atime = 1000;
    const mtime = 50000;
    const f = await Deno.open("README.md", "r");
    try {
      await f.utime(atime, mtime);
    } catch (e) {
      caughtError = true;
      err = e;
    }
    f.close();
    // throw if we lack --write permissions
    assert(caughtError);
    if (caughtError) {
      assert(err instanceof Deno.errors.PermissionDenied);
      assertEquals(err.name, "PermissionDenied");
    }
  });

  testPerm({ read: true, write: true }, function futimeSyncPerm2(): void {
    let err;
    let caughtError = false;
    const atime = 1000;
    const mtime = 50000;
    const filename = Deno.makeTempDirSync() + "/test_utimeSync.txt";
    const f0 = Deno.openSync(filename, "w");
    f0.close();
    const f = Deno.openSync(filename, "r");
    try {
      f.utimeSync(atime, mtime);
    } catch (e) {
      caughtError = true;
      err = e;
    }
    f.close();
    // throw if fd is not opened for writing
    assert(caughtError);
    if (caughtError) {
      assert(err instanceof Deno.errors.PermissionDenied);
      assertEquals(err.name, "PermissionDenied");
    }
  });

  testPerm({ read: true, write: true }, async function futimePerm2(): Promise<
    void
  > {
    let err;
    let caughtError = false;
    const atime = 1000;
    const mtime = 50000;
    const filename = (await Deno.makeTempDir()) + "/test_utime.txt";
    const f0 = await Deno.open(filename, "w");
    f0.close();
    const f = await Deno.open(filename, "r");
    try {
      await f.utime(atime, mtime);
    } catch (e) {
      caughtError = true;
      err = e;
    }
    f.close();
    // throw if fd is not opened for writing
    assert(caughtError);
    if (caughtError) {
      assert(err instanceof Deno.errors.PermissionDenied);
      assertEquals(err.name, "PermissionDenied");
    }
  });
}
