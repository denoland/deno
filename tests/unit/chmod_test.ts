// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

let modeAsync: number;
let modeSync: number;
// On Windows chmod is only able to manipulate write permission
if (Deno.build.os === "windows") {
  modeAsync = 0o444; // read-only
  modeSync = 0o666; // read-write
} else {
  modeAsync = 0o777;
  modeSync = 0o644;
}

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  function chmodSyncSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });

    Deno.chmodSync(filename, modeSync);

    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, modeSync);
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  function chmodSyncUrl() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${tempDir}/test.txt`);
    Deno.writeFileSync(fileUrl, data, { mode: 0o666 });

    Deno.chmodSync(fileUrl, modeSync);

    const fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, modeSync);

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test({ permissions: { write: true } }, function chmodSyncFailure() {
  const filename = "/badfile.txt";
  assertThrows(
    () => {
      Deno.chmodSync(filename, 0o777);
    },
    Deno.errors.NotFound,
    `chmod '${filename}'`,
  );
});

Deno.test({ permissions: { write: false } }, function chmodSyncPerm() {
  assertThrows(() => {
    Deno.chmodSync("/somefile.txt", 0o777);
  }, Deno.errors.NotCapable);
});

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  async function chmodSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });

    await Deno.chmod(filename, modeAsync);

    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, modeAsync);
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  async function chmodUrl() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${tempDir}/test.txt`);
    Deno.writeFileSync(fileUrl, data, { mode: 0o666 });

    await Deno.chmod(fileUrl, modeAsync);

    const fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, modeAsync);

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test({ permissions: { write: true } }, async function chmodFailure() {
  const filename = "/badfile.txt";
  await assertRejects(
    async () => {
      await Deno.chmod(filename, 0o777);
    },
    Deno.errors.NotFound,
    `chmod '${filename}'`,
  );
});

Deno.test({ permissions: { write: false } }, async function chmodPerm() {
  await assertRejects(async () => {
    await Deno.chmod("/somefile.txt", 0o777);
  }, Deno.errors.NotCapable);
});

// chmod does not follow a terminal symlink: the permission check is performed
// no-follow, so a symlink at an allowed path must not be usable to change the
// mode of a file it points to. The op opens the path with O_NOFOLLOW and fails
// with FilesystemLoop (ELOOP) on a symlink target, leaving the target's mode
// untouched.
Deno.test(
  {
    permissions: { read: true, write: true },
    ignore: Deno.build.os === "windows",
  },
  function chmodSyncDoesNotFollowSymlink() {
    const dir = Deno.makeTempDirSync();
    const target = dir + "/target.txt";
    const link = dir + "/link.txt";
    Deno.writeFileSync(target, new Uint8Array([1, 2, 3]));
    Deno.chmodSync(target, 0o600);
    Deno.symlinkSync(target, link);

    assertThrows(
      () => Deno.chmodSync(link, 0o777),
      Deno.errors.FilesystemLoop,
    );
    assertEquals(Deno.lstatSync(target).mode! & 0o777, 0o600);
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
    ignore: Deno.build.os === "windows",
  },
  async function chmodDoesNotFollowSymlink() {
    const dir = Deno.makeTempDirSync();
    const target = dir + "/target.txt";
    const link = dir + "/link.txt";
    await Deno.writeFile(target, new Uint8Array([1, 2, 3]));
    await Deno.chmod(target, 0o600);
    await Deno.symlink(target, link);

    await assertRejects(
      () => Deno.chmod(link, 0o777),
      Deno.errors.FilesystemLoop,
    );
    assertEquals(Deno.lstatSync(target).mode! & 0o777, 0o600);
  },
);

// O_NOFOLLOW only protects the final path component. A symlink in an ancestor
// directory is still resolved, so chmod-ing a regular file reached through a
// symlinked directory continues to work.
Deno.test(
  {
    permissions: { read: true, write: true },
    ignore: Deno.build.os === "windows",
  },
  function chmodSyncFollowsIntermediateSymlink() {
    const dir = Deno.makeTempDirSync();
    const realDir = dir + "/real";
    Deno.mkdirSync(realDir);
    const file = realDir + "/data.txt";
    Deno.writeFileSync(file, new Uint8Array([1, 2, 3]));
    Deno.chmodSync(file, 0o600);
    const linkDir = dir + "/linkdir";
    Deno.symlinkSync(realDir, linkDir);

    Deno.chmodSync(linkDir + "/data.txt", 0o755);
    assertEquals(Deno.lstatSync(file).mode! & 0o777, 0o755);
  },
);
