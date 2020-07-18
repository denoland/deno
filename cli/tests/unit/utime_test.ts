// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertThrows,
  assertThrowsAsync,
} from "./test_util.ts";

// Allow 10 second difference.
// Note this might not be enough for FAT (but we are not testing on such fs).
function assertFuzzyTimestampEquals(t1: Date | null, t2: Date): void {
  assert(t1 instanceof Date);
  assert(Math.abs(t1.valueOf() - t2.valueOf()) < 10_000);
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
    assertFuzzyTimestampEquals(fileInfo.atime, new Date(atime * 1000));
    assertFuzzyTimestampEquals(fileInfo.mtime, new Date(mtime * 1000));
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function utimeSyncDirectorySuccess(): void {
    const testDir = Deno.makeTempDirSync();

    const atime = 1000;
    const mtime = 50000;
    Deno.utimeSync(testDir, atime, mtime);

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.atime, new Date(atime * 1000));
    assertFuzzyTimestampEquals(dirInfo.mtime, new Date(mtime * 1000));
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function utimeSyncDateSuccess(): void {
    const testDir = Deno.makeTempDirSync();

    const atime = new Date(1000_000);
    const mtime = new Date(50000_000);
    Deno.utimeSync(testDir, atime, mtime);

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.atime, atime);
    assertFuzzyTimestampEquals(dirInfo.mtime, mtime);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function utimeSyncFileDateSuccess() {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
      mode: 0o666,
    });
    const atime = new Date();
    const mtime = new Date();
    Deno.utimeSync(filename, atime, mtime);

    const fileInfo = Deno.statSync(filename);
    assertFuzzyTimestampEquals(fileInfo.atime, atime);
    assertFuzzyTimestampEquals(fileInfo.mtime, mtime);
  },
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
    assertFuzzyTimestampEquals(dirInfo.atime, new Date(atime * 1000));
    assertFuzzyTimestampEquals(dirInfo.mtime, new Date(mtime * 1000));
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function utimeSyncNotFound(): void {
    const atime = 1000;
    const mtime = 50000;

    assertThrows(() => {
      Deno.utimeSync("/baddir", atime, mtime);
    }, Deno.errors.NotFound);
  },
);

unitTest(
  { perms: { read: true, write: false } },
  function utimeSyncPerm(): void {
    const atime = 1000;
    const mtime = 50000;

    assertThrows(() => {
      Deno.utimeSync("/some_dir", atime, mtime);
    }, Deno.errors.PermissionDenied);
  },
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
    assertFuzzyTimestampEquals(fileInfo.atime, new Date(atime * 1000));
    assertFuzzyTimestampEquals(fileInfo.mtime, new Date(mtime * 1000));
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function utimeDirectorySuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();

    const atime = 1000;
    const mtime = 50000;
    await Deno.utime(testDir, atime, mtime);

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.atime, new Date(atime * 1000));
    assertFuzzyTimestampEquals(dirInfo.mtime, new Date(mtime * 1000));
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function utimeDateSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();

    const atime = new Date(100_000);
    const mtime = new Date(5000_000);
    await Deno.utime(testDir, atime, mtime);

    const dirInfo = Deno.statSync(testDir);
    assertFuzzyTimestampEquals(dirInfo.atime, atime);
    assertFuzzyTimestampEquals(dirInfo.mtime, mtime);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function utimeFileDateSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    Deno.writeFileSync(filename, new TextEncoder().encode("hello"), {
      mode: 0o666,
    });

    const atime = new Date();
    const mtime = new Date();
    await Deno.utime(filename, atime, mtime);

    const fileInfo = Deno.statSync(filename);
    assertFuzzyTimestampEquals(fileInfo.atime, atime);
    assertFuzzyTimestampEquals(fileInfo.mtime, mtime);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function utimeNotFound(): Promise<void> {
    const atime = 1000;
    const mtime = 50000;

    await assertThrowsAsync(async () => {
      await Deno.utime("/baddir", atime, mtime);
    }, Deno.errors.NotFound);
  },
);

unitTest(
  { perms: { read: true, write: false } },
  async function utimeSyncPerm(): Promise<void> {
    const atime = 1000;
    const mtime = 50000;

    await assertThrowsAsync(async () => {
      await Deno.utime("/some_dir", atime, mtime);
    }, Deno.errors.PermissionDenied);
  },
);
