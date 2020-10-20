import { BigIntStats, stat, Stats, statSync } from "./_fs_stat.ts";
import { assertEquals, fail } from "../../testing/asserts.ts";

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
  assertEquals(actual.atimeMs, expected.atime?.getTime());
  assertEquals(actual.mtimeMs, expected.mtime?.getTime());
  assertEquals(actual.birthtimeMs, expected.birthtime?.getTime());
  assertEquals(actual.isFile(), expected.isFile);
  assertEquals(actual.isDirectory(), expected.isDirectory);
  assertEquals(actual.isSymbolicLink(), expected.isSymlink);
}

function to_BigInt(num?: number | null) {
  if (num === undefined || num === null) return null;
  return BigInt(num);
}

export function assertStatsBigInt(
  actual: BigIntStats,
  expected: Deno.FileInfo,
) {
  assertEquals(actual.dev, to_BigInt(expected.dev));
  assertEquals(actual.gid, to_BigInt(expected.gid));
  assertEquals(actual.size, to_BigInt(expected.size));
  assertEquals(actual.blksize, to_BigInt(expected.blksize));
  assertEquals(actual.blocks, to_BigInt(expected.blocks));
  assertEquals(actual.ino, to_BigInt(expected.ino));
  assertEquals(actual.gid, to_BigInt(expected.gid));
  assertEquals(actual.mode, to_BigInt(expected.mode));
  assertEquals(actual.nlink, to_BigInt(expected.nlink));
  assertEquals(actual.rdev, to_BigInt(expected.rdev));
  assertEquals(actual.uid, to_BigInt(expected.uid));
  assertEquals(actual.atime?.getTime(), expected.atime?.getTime());
  assertEquals(actual.mtime?.getTime(), expected.mtime?.getTime());
  assertEquals(actual.birthtime?.getTime(), expected.birthtime?.getTime());
  assertEquals(Number(actual.atimeMs), expected.atime?.getTime());
  assertEquals(Number(actual.mtimeMs), expected.mtime?.getTime());
  assertEquals(Number(actual.birthtimeMs), expected.birthtime?.getTime());
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
      .then((stat) => assertStats(stat, Deno.statSync(file)))
      .catch(() => fail())
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
      .then((stat) => assertStatsBigInt(stat, Deno.statSync(file)))
      .catch(() => fail())
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
