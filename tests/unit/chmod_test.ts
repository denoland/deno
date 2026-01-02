// Copyright 2018-2025 the Deno authors. MIT license.
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

// Check symlink when not on windows
Deno.test(
  {
    permissions: { read: true, write: true },
  },
  function chmodSyncSymlinkSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const symlinkName = tempDir + "/test_symlink.txt";
    Deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    Deno.chmodSync(symlinkName, modeSync);

    // Change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, modeSync);

    symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
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

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  async function chmodSymlinkSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const symlinkName = tempDir + "/test_symlink.txt";
    Deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    await Deno.chmod(symlinkName, modeAsync);

    // Just change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, modeAsync);

    symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
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
