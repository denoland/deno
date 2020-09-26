import { BigIntStats, stat, Stats, statSync } from "./_fs_stat.ts";
import { assert, assertThrows } from "../../testing/asserts.ts";

export function assertStats(actual: Stats, expected: Deno.FileInfo) {
  assert(actual.dev === expected.dev);
  assert(actual.gid === expected.gid);
  assert(actual.size === expected.size);
  assert(actual.blksize === expected.blksize);
  assert(actual.blocks === expected.blocks);
  assert(actual.ino === expected.ino);
  assert(actual.gid === expected.gid);
  assert(actual.mode === expected.mode);
  assert(actual.nlink === expected.nlink);
  assert(actual.rdev === expected.rdev);
  assert(actual.uid === expected.uid);
  assert(actual.atime?.getTime() === expected.atime?.getTime());
  assert(actual.mtime?.getTime() === expected.mtime?.getTime());
  assert(actual.birthtime?.getTime() === expected.birthtime?.getTime());
  assert(actual.atimeMs === expected.atime?.getTime());
  assert(actual.mtimeMs === expected.mtime?.getTime());
  assert(actual.birthtimeMs === expected.birthtime);
  assert(actual.isFile() === expected.isFile);
  assert(actual.isDirectory() === expected.isDirectory);
  assert(actual.isSymbolicLink() === expected.isSymlink);
  // assert(actual.ctimeMs === expected.ctime?.getTime());
  // assert(actual.ctime?.getTime() === expected.ctime?.getTime());
}

export function assertStatsBigInt(
  actual: BigIntStats,
  expected: Deno.FileInfo
) {
  assert(actual.dev === BigInt(expected.dev));
  assert(actual.gid === BigInt(expected.gid));
  assert(actual.size === BigInt(expected.size));
  assert(actual.blksize === BigInt(expected.blksize));
  assert(actual.blocks === BigInt(expected.blocks));
  assert(actual.ino === BigInt(expected.ino));
  assert(actual.gid === BigInt(expected.gid));
  assert(actual.mode === BigInt(expected.mode));
  assert(actual.nlink === BigInt(expected.nlink));
  assert(actual.rdev === BigInt(expected.rdev));
  assert(actual.uid === BigInt(expected.uid));
  assert(actual.atime?.getTime() === expected.atime?.getTime());
  assert(actual.mtime?.getTime() === expected.mtime?.getTime());
  assert(actual.birthtime?.getTime() === expected.birthtime?.getTime());
  assert(Number(actual.atimeMs) === expected.atime?.getTime());
  assert(Number(actual.mtimeMs) === expected.mtime?.getTime());
  assert(Number(actual.birthtimeMs) === expected.birthtime?.getTime());
  assert(Number(actual.atimeNs) / 1e6 === expected.atime?.getTime());
  assert(Number(actual.mtimeNs) / 1e6 === expected.atime?.getTime());
  assert(Number(actual.birthtimeNs) / 1e6 === expected.atime?.getTime());
  assert(actual.isFile() === expected.isFile);
  assert(actual.isDirectory() === expected.isDirectory);
  assert(actual.isSymbolicLink() === expected.isSymlink);
  // assert(actual.ctime?.getTime() === expected.ctime?.getTime());
  // assert(Number(actual.ctimeMs) === expected.ctime?.getTime());
  // assert(Number(actual.ctimeNs) / 1e+6 === expected.ctime?.getTime());
}

Deno.test({
  name: "No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-ignore
        stat(Deno.makeTempFileSync());
      },
      Error,
      "No callback function supplied"
    );
  },
});

Deno.test({
  name: "Test Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    stat(file, (err, stat) => {
      if (err) throw err;
      assertStats(stat, Deno.statSync(file));
    });
  },
});

Deno.test({
  name: "Test Stats (sync)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStats(statSync(file), Deno.statSync(file));
  },
});

Deno.test({
  name: "Test BigInt Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    stat(file, { bigint: true }, (err, stat) => {
      if (err) throw err;
      assertStatsBigInt(stat, Deno.statSync(file));
    });
  },
});

Deno.test({
  name: "Test BigInt Stats (sync)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStatsBigInt(statSync(file, { bigint: true }), Deno.statSync(file));
  },
});
