// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { BigIntStats, stat, Stats, statSync } from "node:fs";
import { assertEquals, fail } from "@std/assert";

export function assertStats(actual: Stats, expected: Deno.FileInfo) {
  assertEquals(actual.dev, expected.dev);
  assertEquals(actual.gid, expected.gid);
  assertEquals(actual.size, expected.size);
  assertEquals(actual.blksize, expected.blksize);
  assertEquals(actual.blocks, expected.blocks);
  assertEquals(actual.ino, expected.ino);
  assertEquals(actual.gid, expected.gid);
  assertEquals(actual.mode, expected.mode);
  assertEquals(actual.nlink, expected.nlink);
  assertEquals(actual.rdev, expected.rdev);
  assertEquals(actual.uid, expected.uid);
  assertEquals(actual.atime?.getTime(), expected.atime?.getTime());
  assertEquals(actual.mtime?.getTime(), expected.mtime?.getTime());
  assertEquals(actual.birthtime?.getTime(), expected.birthtime?.getTime());
  assertEquals(actual.ctime?.getTime(), expected.ctime?.getTime());
  assertEquals(actual.atimeMs ?? undefined, expected.atime?.getTime());
  assertEquals(actual.mtimeMs ?? undefined, expected.mtime?.getTime());
  assertEquals(actual.birthtimeMs ?? undefined, expected.birthtime?.getTime());
  assertEquals(actual.ctimeMs ?? undefined, expected.ctime?.getTime());
  assertEquals(actual.isFile(), expected.isFile);
  assertEquals(actual.isDirectory(), expected.isDirectory);
  assertEquals(actual.isSymbolicLink(), expected.isSymlink);
}

function toBigInt(num?: number | null) {
  if (num === undefined || num === null) return null;
  return BigInt(num);
}

export function assertStatsBigInt(
  actual: BigIntStats,
  expected: Deno.FileInfo,
) {
  assertEquals(actual.dev, toBigInt(expected.dev));
  assertEquals(actual.gid, toBigInt(expected.gid));
  assertEquals(actual.size, toBigInt(expected.size));
  assertEquals(actual.blksize, toBigInt(expected.blksize));
  assertEquals(actual.blocks, toBigInt(expected.blocks));
  assertEquals(actual.ino, toBigInt(expected.ino));
  assertEquals(actual.gid, toBigInt(expected.gid));
  assertEquals(actual.mode, toBigInt(expected.mode));
  assertEquals(actual.nlink, toBigInt(expected.nlink));
  assertEquals(actual.rdev, toBigInt(expected.rdev));
  assertEquals(actual.uid, toBigInt(expected.uid));
  assertEquals(actual.atime?.getTime(), expected.atime?.getTime());
  assertEquals(actual.mtime?.getTime(), expected.mtime?.getTime());
  assertEquals(actual.birthtime?.getTime(), expected.birthtime?.getTime());
  assertEquals(actual.ctime?.getTime(), expected.ctime?.getTime());
  assertEquals(
    actual.atimeMs === null ? undefined : Number(actual.atimeMs),
    expected.atime?.getTime(),
  );
  assertEquals(
    actual.mtimeMs === null ? undefined : Number(actual.mtimeMs),
    expected.mtime?.getTime(),
  );
  assertEquals(
    actual.birthtimeMs === null ? undefined : Number(actual.birthtimeMs),
    expected.birthtime?.getTime(),
  );
  assertEquals(
    actual.ctimeMs === null ? undefined : Number(actual.ctimeMs),
    expected.ctime?.getTime(),
  );
  assertEquals(actual.atimeNs === null, actual.atime === null);
  assertEquals(actual.mtimeNs === null, actual.mtime === null);
  assertEquals(actual.birthtimeNs === null, actual.birthtime === null);
  assertEquals(actual.isFile(), expected.isFile);
  assertEquals(actual.isDirectory(), expected.isDirectory);
  assertEquals(actual.isSymbolicLink(), expected.isSymlink);
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
