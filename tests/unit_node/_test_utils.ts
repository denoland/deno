// Copyright 2018-2025 the Deno authors. MIT license.

import { BigIntStats, Stats } from "node:fs";
import {
  assert,
  assertEquals,
  assertNotStrictEquals,
  assertStrictEquals,
  assertStringIncludes,
} from "@std/assert";

/** Asserts that an error thrown in a callback will not be wrongly caught. */
export async function assertCallbackErrorUncaught(
  { prelude, invocation, cleanup }: {
    /** Any code which needs to run before the actual invocation (notably, any import statements). */
    prelude?: string;
    /**
     * The start of the invocation of the function, e.g. `open("foo.txt", `.
     * The callback will be added after it.
     */
    invocation: string;
    /** Called after the subprocess is finished but before running the assertions, e.g. to clean up created files. */
    cleanup?: () => Promise<void> | void;
  },
) {
  // Since the error has to be uncaught, and that will kill the Deno process,
  // the only way to test this is to spawn a subprocess.
  const p = new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      `${prelude ?? ""}
  
        ${invocation}(err) => {
          // If the bug is present and the callback is called again with an error,
          // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
          if (!err) throw new Error("success");
        });`,
    ],
    stderr: "piped",
  });
  const { stderr, success } = await p.output();
  const error = new TextDecoder().decode(stderr);
  await cleanup?.();
  assert(!success);
  assertStringIncludes(error, "Error: success");
}

const numberFields = [
  "dev",
  "mode",
  "nlink",
  "uid",
  "gid",
  "rdev",
  "blksize",
  "ino",
  "size",
  "blocks",
  "atimeMs",
  "mtimeMs",
  "ctimeMs",
  "birthtimeMs",
];

const bigintFields = [
  ...numberFields,
  "atimeNs",
  "mtimeNs",
  "ctimeNs",
  "birthtimeNs",
];

const dateFields = [
  "atime",
  "mtime",
  "ctime",
  "birthtime",
];

export function assertStats(actual: Stats, expected: Deno.FileInfo) {
  [...numberFields, ...dateFields].forEach(function (k) {
    assert(k in actual, `${k} should be in Stats`);
    assertNotStrictEquals(
      actual[k as keyof Stats],
      undefined,
      `${k} should not be undefined`,
    );
    assertNotStrictEquals(
      actual[k as keyof Stats],
      null,
      `${k} should not be null`,
    );
  });

  numberFields.forEach((k) => {
    assertStrictEquals(
      typeof actual[k as keyof Stats],
      "number",
      `${k} should be a number`,
    );
  });

  dateFields.forEach((k) => {
    assert(actual[k as keyof Stats] instanceof Date, `${k} should be a Date`);
  });

  // Some properties from Deno.FileInfo may be null,
  // while node:fs Stats always has number / Date properties.
  // So we only check properties that are not null in Deno.FileInfo.
  assertEquals(actual.dev, expected.dev);
  if (expected.gid) assertEquals(actual.gid, expected.gid, "Stats.gid");
  assertEquals(actual.size, expected.size, "Stats.size");
  if (expected.blksize) {
    assertEquals(actual.blksize, expected.blksize, "Stats.blksize");
  }
  if (expected.blocks) {
    assertEquals(actual.blocks, expected.blocks, "Stats.blocks");
  }
  if (expected.ino) assertEquals(actual.ino, expected.ino, "Stats.ino");
  if (expected.mode) assertEquals(actual.mode, expected.mode, "Stats.mode");
  if (expected.nlink) assertEquals(actual.nlink, expected.nlink, "Stats.nlink");
  if (expected.rdev) assertEquals(actual.rdev, expected.rdev, "Stats.rdev");
  if (expected.uid) assertEquals(actual.uid, expected.uid, "Stats.uid");
  if (expected.atime?.getTime()) {
    assertEquals(
      actual.atime.getTime(),
      expected.atime.getTime(),
      "Stats.atime",
    );
    assertEquals(actual.atimeMs, expected.atime.getTime(), "Stats.atimeMs");
  }
  if (expected.mtime?.getTime()) {
    assertEquals(
      actual.mtime.getTime(),
      expected.mtime.getTime(),
      "Stats.mtime",
    );
    assertEquals(actual.mtimeMs, expected.mtime.getTime(), "Stats.mtimeMs");
  }
  if (expected.birthtime?.getTime()) {
    assertEquals(
      actual.birthtime.getTime(),
      expected.birthtime.getTime(),
      "Stats.birthtime",
    );
    assertEquals(
      actual.birthtimeMs,
      expected.birthtime.getTime(),
      "Stats.birthtimeMs",
    );
  }
  if (expected.ctime?.getTime()) {
    assertEquals(
      actual.ctime.getTime(),
      expected.ctime.getTime(),
      "Stats.ctime",
    );
    assertEquals(actual.ctimeMs, expected.ctime.getTime(), "Stats.ctimeMs");
  }
  assertEquals(actual.isFile(), expected.isFile, "Stats.isFile");
  assertEquals(actual.isDirectory(), expected.isDirectory, "Stats.isDirectory");
  assertEquals(
    actual.isSymbolicLink(),
    expected.isSymlink,
    "Stats.isSymbolicLink",
  );
  assertEquals(
    actual.isBlockDevice(),
    Deno.build.os === "windows" ? false : expected.isBlockDevice,
    "Stats.isBlockDevice",
  );
  assertEquals(
    actual.isFIFO(),
    Deno.build.os === "windows" ? false : expected.isFifo,
    "Stats.isFIFO",
  );
  assertEquals(
    actual.isCharacterDevice(),
    Deno.build.os === "windows" ? false : expected.isCharDevice,
    "Stats.isCharacterDevice",
  );
  assertEquals(
    actual.isSocket(),
    Deno.build.os === "windows" ? false : expected.isSocket,
    "Stats.isSocket",
  );
}

function toBigInt(num?: number | null) {
  if (num === undefined || num === null) return null;
  return BigInt(num);
}

export function assertStatsBigInt(
  actual: BigIntStats,
  expected: Deno.FileInfo,
) {
  [...bigintFields, ...dateFields].forEach(function (k) {
    assert(k in actual, `${k} should be in BigIntStats`);
    assertNotStrictEquals(
      actual[k as keyof BigIntStats],
      undefined,
      `${k} should not be undefined`,
    );
    assertNotStrictEquals(
      actual[k as keyof BigIntStats],
      null,
      `${k} should not be null`,
    );
  });

  bigintFields.forEach((k) => {
    assertStrictEquals(
      typeof actual[k as keyof BigIntStats],
      "bigint",
      `${k} should be a bigint`,
    );
  });

  dateFields.forEach((k) => {
    assert(
      actual[k as keyof BigIntStats] instanceof Date,
      `${k} should be a Date`,
    );
  });

  // Some properties from Deno.FileInfo may be null,
  // while node:fs BigIntStats always has bigint / Date properties.
  // So we only check properties that are not null in Deno.FileInfo.
  assertEquals(actual.dev, toBigInt(expected.dev), "BigIntStats.dev");
  if (expected.gid) {
    assertEquals(actual.gid, toBigInt(expected.gid), "BigIntStats.gid");
  }
  assertEquals(actual.size, toBigInt(expected.size), "BigIntStats.size");
  if (expected.blksize) {
    assertEquals(
      actual.blksize,
      toBigInt(expected.blksize),
      "BigIntStats.blksize",
    );
  }
  if (expected.blocks) {
    assertEquals(
      actual.blocks,
      toBigInt(expected.blocks),
      "BigIntStats.blocks",
    );
  }
  if (expected.ino) {
    assertEquals(actual.ino, toBigInt(expected.ino), "BigIntStats.ino");
  }
  if (expected.mode) {
    assertEquals(actual.mode, toBigInt(expected.mode), "BigIntStats.mode");
  }
  if (expected.nlink) {
    assertEquals(actual.nlink, toBigInt(expected.nlink), "BigIntStats.nlink");
  }
  if (expected.rdev) {
    assertEquals(actual.rdev, toBigInt(expected.rdev), "BigIntStats.rdev");
  }
  if (expected.uid) {
    assertEquals(actual.uid, toBigInt(expected.uid), "BigIntStats.uid");
  }
  if (expected.atime?.getTime()) {
    assertEquals(
      actual.atime.getTime(),
      expected.atime.getTime(),
      "BigIntStats.atime",
    );
    assertEquals(
      actual.atimeMs,
      toBigInt(expected.atime.getTime()),
      "BigIntStats.atimeMs",
    );
    assertEquals(
      actual.atimeNs,
      toBigInt(expected.atime.getTime()) as bigint * 1000000n,
      "BigIntStats.atimeNs",
    );
  }
  if (expected.mtime?.getTime()) {
    assertEquals(
      actual.mtime.getTime(),
      expected.mtime.getTime(),
      "BigIntStats.mtime",
    );
    assertEquals(
      actual.mtimeMs,
      toBigInt(expected.mtime.getTime()),
      "BigIntStats.mtimeMs",
    );
    assertEquals(
      actual.mtimeNs,
      toBigInt(expected.mtime.getTime()) as bigint * 1000000n,
      "BigIntStats.mtimeNs",
    );
  }
  if (expected.birthtime?.getTime()) {
    assertEquals(
      actual.birthtime.getTime(),
      expected.birthtime.getTime(),
      "BigIntStats.birthtime",
    );
    assertEquals(
      actual.birthtimeMs,
      toBigInt(expected.birthtime.getTime()),
      "BigIntStats.birthtimeMs",
    );
    assertEquals(
      actual.birthtimeNs,
      toBigInt(expected.birthtime.getTime()) as bigint * 1000000n,
      "BigIntStats.birthtimeNs",
    );
  }
  if (expected.ctime?.getTime()) {
    assertEquals(
      actual.ctime.getTime(),
      expected.ctime.getTime(),
      "BigIntStats.ctime",
    );
    assertEquals(
      actual.ctimeMs,
      toBigInt(expected.ctime.getTime()),
      "BigIntStats.ctimeMs",
    );
    assertEquals(
      actual.ctimeNs,
      toBigInt(expected.ctime.getTime()) as bigint * 1000000n,
      "BigIntStats.ctimeNs",
    );
  }
  assertEquals(
    actual.isBlockDevice(),
    Deno.build.os === "windows" ? false : expected.isBlockDevice,
    "BigIntStats.isBlockDevice",
  );
  assertEquals(
    actual.isFIFO(),
    Deno.build.os === "windows" ? false : expected.isFifo,
    "BigIntStats.isFIFO",
  );
  assertEquals(
    actual.isCharacterDevice(),
    Deno.build.os === "windows" ? false : expected.isCharDevice,
    "BigIntStats.isCharacterDevice",
  );
  assertEquals(
    actual.isSocket(),
    Deno.build.os === "windows" ? false : expected.isSocket,
    "BigIntStats.isSocket",
  );
}
