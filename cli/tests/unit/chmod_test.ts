// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "./test_util.ts";

Deno.test({
  name: "chmodSyncSuccess",
  ignore: Deno.build.os === "windows",
  fn(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });

    Deno.chmodSync(filename, 0o777);

    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
  },
});

Deno.test({
  name: "chmodSyncUrl",
  ignore: Deno.build.os === "windows",
  fn(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${tempDir}/test.txt`);
    Deno.writeFileSync(fileUrl, data, { mode: 0o666 });

    Deno.chmodSync(fileUrl, 0o777);

    const fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);

    Deno.removeSync(tempDir, { recursive: true });
  },
});

Deno.test({
  name: "chmodSyncSymlinkSuccess",
  ignore: Deno.build.os === "windows",
  fn(): void {
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

    Deno.chmodSync(symlinkName, 0o777);

    // Change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
  },
});

Deno.test("chmodSyncFailure", function (): void {
  assertThrows(() => {
    const filename = "/badfile.txt";
    Deno.chmodSync(filename, 0o777);
  }, Deno.errors.NotFound);
});

Deno.test({
  name: "chmodSuccess",
  ignore: Deno.build.os === "windows",
  async fn(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });

    await Deno.chmod(filename, 0o777);

    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
  },
});

Deno.test({
  name: "chmodUrl",
  ignore: Deno.build.os === "windows",
  async fn(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${tempDir}/test.txt`);
    Deno.writeFileSync(fileUrl, data, { mode: 0o666 });

    await Deno.chmod(fileUrl, 0o777);

    const fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);

    Deno.removeSync(tempDir, { recursive: true });
  },
});

Deno.test({
  name: "chmodSymlinkSuccess",
  ignore: Deno.build.os === "windows",
  async fn(): Promise<void> {
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

    await Deno.chmod(symlinkName, 0o777);

    // Just change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
  },
});

Deno.test("chmodFailure", async function (): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    const filename = "/badfile.txt";
    await Deno.chmod(filename, 0o777);
  }, Deno.errors.NotFound);
});

Deno.test("chmodSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "write" });

  assertThrows(() => {
    Deno.chmodSync("/somefile.txt", 0o777);
  }, Deno.errors.PermissionDenied);
});

Deno.test("chmodPerm", async function (): Promise<
  void
> {
  await Deno.permissions.revoke({ name: "write" });

  await assertThrowsAsync(async () => {
    await Deno.chmod("/somefile.txt", 0o777);
  }, Deno.errors.PermissionDenied);
});
