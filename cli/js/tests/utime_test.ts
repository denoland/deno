// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

// Allow 10 second difference.
// Note this might not be enough for FAT (but we are not testing on such fs).
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function assertFuzzyTimestampEquals(t1: any, t2: number): void {
  assert(typeof t1 === "number");
  assert(Math.abs(t1 - t2) < 10);
}

unitTest(
  { perms: { read: true, write: true } },
  function utimeSyncFileSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
      mode: 0o666,
    });

    const atime = 1000;
    const mtime = 50000;
    Deno.utimeSync(filename, atime, mtime);

    const fileInfo = Deno.statSync(filename);
    assertFuzzyTimestampEquals(fileInfo.accessed, atime);
    assertFuzzyTimestampEquals(fileInfo.modified, mtime);
  }
);

unitTest(
  { perms: { read: true, write: true } },
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

unitTest(
  { perms: { read: true, write: true } },
  function utimeSyncDateSuccess(): void {
    const testDir = Deno.makeTempDirSync();

    const atime = 1000;
    const mtime = 50000;
    Deno.utimeSync(testDir, new Date(atime * 1000), new Date(mtime * 1000));

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.accessed, atime);
    assertFuzzyTimestampEquals(dirInfo.modified, mtime);
  }
);

unitTest(
  { perms: { read: true, write: true } },
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

unitTest(
  { perms: { read: true, write: true } },
  function utimeSyncNotFound(): void {
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
  }
);

unitTest(
  { perms: { read: true, write: false } },
  function utimeSyncPerm(): void {
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
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function utimeFileSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
      mode: 0o666,
    });

    const atime = 1000;
    const mtime = 50000;
    await Deno.utime(filename, atime, mtime);

    const fileInfo = Deno.statSync(filename);
    assertFuzzyTimestampEquals(fileInfo.accessed, atime);
    assertFuzzyTimestampEquals(fileInfo.modified, mtime);
  }
);

unitTest(
  { perms: { read: true, write: true } },
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

unitTest(
  { perms: { read: true, write: true } },
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

unitTest(
  { perms: { read: true, write: true } },
  async function utimeNotFound(): Promise<void> {
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
  }
);

unitTest(
  { perms: { read: true, write: false } },
  async function utimeSyncPerm(): Promise<void> {
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
  }
);
