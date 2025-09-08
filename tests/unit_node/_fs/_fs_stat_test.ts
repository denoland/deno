// Copyright 2018-2025 the Deno authors. MIT license.
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { BigIntStats, stat, Stats, statSync } from "node:fs";
import {
  assert,
  assertEquals,
  assertNotStrictEquals,
  assertStrictEquals,
  fail,
} from "@std/assert";
import { isWindows } from "../../util/std/path/_os.ts";

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

function assertStats(actual: Stats, expected: Deno.FileInfo) {
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
    isWindows ? false : expected.isBlockDevice,
    "Stats.isBlockDevice",
  );
  assertEquals(
    actual.isFIFO(),
    isWindows ? false : expected.isFifo,
    "Stats.isFIFO",
  );
  assertEquals(
    actual.isCharacterDevice(),
    isWindows ? false : expected.isCharDevice,
    "Stats.isCharacterDevice",
  );
  assertEquals(
    actual.isSocket(),
    isWindows ? false : expected.isSocket,
    "Stats.isSocket",
  );
}

function toBigInt(num?: number | null) {
  if (num === undefined || num === null) return null;
  return BigInt(num);
}

function assertStatsBigInt(
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
    isWindows ? false : expected.isBlockDevice,
    "BigIntStats.isBlockDevice",
  );
  assertEquals(
    actual.isFIFO(),
    isWindows ? false : expected.isFifo,
    "BigIntStats.isFIFO",
  );
  assertEquals(
    actual.isCharacterDevice(),
    isWindows ? false : expected.isCharDevice,
    "BigIntStats.isCharacterDevice",
  );
  assertEquals(
    actual.isSocket(),
    isWindows ? false : expected.isSocket,
    "BigIntStats.isSocket",
  );
}

Deno.test({
  name: "ASYNC: get a file Stats",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<Stats>((resolve, reject) => {
      stat(file, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then((stat) => assertStats(stat, Deno.statSync(file)), () => fail())
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: get a file Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStats(statSync(file), Deno.statSync(file));
  },
});

Deno.test({
  name: "ASYNC: get a file BigInt Stats",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<BigIntStats>((resolve, reject) => {
      stat(file, { bigint: true }, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => assertStatsBigInt(stat, Deno.statSync(file)),
        () => fail(),
      )
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: get a file BigInt Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStatsBigInt(statSync(file, { bigint: true }), Deno.statSync(file));
  },
});

Deno.test("[std/node/fs] stat callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { stat } from ${JSON.stringify(importUrl)}`,
    invocation: `stat(${JSON.stringify(tempFile)}, `,
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});

Deno.test({
  name: "[std/node/fs] stat default methods",
  fn() {
    // stats ctor is private
    // deno-lint-ignore no-explicit-any
    const stats = new (Stats as any)();
    assertEquals(stats.isFile(), false);
    assertEquals(stats.isDirectory(), false);
    assertEquals(stats.isBlockDevice(), false);
    assertEquals(stats.isCharacterDevice(), false);
    assertEquals(stats.isSymbolicLink(), false);
    assertEquals(stats.isFIFO(), false);
    assertEquals(stats.isSocket(), false);
  },
});

Deno.test({
  name: "[node/fs] stat invalid path error",
  async fn() {
    try {
      await new Promise<Stats>((resolve, reject) => {
        stat(
          // deno-lint-ignore no-explicit-any
          undefined as any,
          (err, stats) => err ? reject(err) : resolve(stats),
        );
      });
      fail();
    } catch (err) {
      assert(err instanceof TypeError);
      // deno-lint-ignore no-explicit-any
      assertEquals((err as any).code, "ERR_INVALID_ARG_TYPE");
    }
  },
});

Deno.test({
  name: "[node/fs] statSync invalid path error",
  fn() {
    try {
      // deno-lint-ignore no-explicit-any
      statSync(undefined as any);
      fail();
    } catch (err) {
      assert(err instanceof TypeError);
      // deno-lint-ignore no-explicit-any
      assertEquals((err as any).code, "ERR_INVALID_ARG_TYPE");
    }
  },
});
