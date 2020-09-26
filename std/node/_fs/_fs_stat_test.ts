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
  // assertEquals(actual.ctimeMs === expected.ctime?.getTime());
  // assertEquals(actual.ctime?.getTime() === expected.ctime?.getTime());
}

export function assertStatsBigInt(
  actual: BigIntStats,
  expected: Deno.FileInfo,
) {
  assertEquals(actual.dev, BigInt(expected.dev));
  assertEquals(actual.gid, BigInt(expected.gid));
  assertEquals(actual.size, BigInt(expected.size));
  assertEquals(actual.blksize, BigInt(expected.blksize));
  assertEquals(actual.blocks, BigInt(expected.blocks));
  assertEquals(actual.ino, BigInt(expected.ino));
  assertEquals(actual.gid, BigInt(expected.gid));
  assertEquals(actual.mode, BigInt(expected.mode));
  assertEquals(actual.nlink, BigInt(expected.nlink));
  assertEquals(actual.rdev, BigInt(expected.rdev));
  assertEquals(actual.uid, BigInt(expected.uid));
  assertEquals(actual.atime?.getTime(), expected.atime?.getTime());
  assertEquals(actual.mtime?.getTime(), expected.mtime?.getTime());
  assertEquals(actual.birthtime?.getTime(), expected.birthtime?.getTime());
  assertEquals(Number(actual.atimeMs), expected.atime?.getTime());
  assertEquals(Number(actual.mtimeMs), expected.mtime?.getTime());
  assertEquals(Number(actual.birthtimeMs), expected.birthtime?.getTime());
  assertEquals(Number(actual.atimeNs) / 1e6, expected.atime?.getTime());
  assertEquals(Number(actual.mtimeNs) / 1e6, expected.atime?.getTime());
  assertEquals(Number(actual.birthtimeNs) / 1e6, expected.atime?.getTime());
  assertEquals(actual.isFile(), expected.isFile);
  assertEquals(actual.isDirectory(), expected.isDirectory);
  assertEquals(actual.isSymbolicLink(), expected.isSymlink);
  // assertEquals(actual.ctime?.getTime() === expected.ctime?.getTime());
  // assertEquals(Number(actual.ctimeMs) === expected.ctime?.getTime());
  // assertEquals(Number(actual.ctimeNs) / 1e+6 === expected.ctime?.getTime());
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
