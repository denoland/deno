// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects, assertThrows } from "./test_util.ts";

// chown on Windows is noop for now, so ignore its testing on Windows

async function getUidAndGid(): Promise<{ uid: number; gid: number }> {
  // get the user ID and group ID of the current process
  const uidProc = await new Deno.Command("id", {
    args: ["-u"],
  }).output();
  const gidProc = await new Deno.Command("id", {
    args: ["-g"],
  }).output();

  assertEquals(uidProc.code, 0);
  assertEquals(gidProc.code, 0);
  const uid = parseInt(new TextDecoder("utf-8").decode(uidProc.stdout));
  const gid = parseInt(new TextDecoder("utf-8").decode(gidProc.stdout));

  return { uid, gid };
}

Deno.test(
  { ignore: Deno.build.os == "windows", permissions: { write: false } },
  async function chownNoWritePermission() {
    const filePath = "chown_test_file.txt";
    await assertRejects(async () => {
      await Deno.chown(filePath, 1000, 1000);
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  {
    permissions: { run: true, write: true },
    ignore: Deno.build.os == "windows",
  },
  async function chownSyncFileNotExist() {
    const { uid, gid } = await getUidAndGid();
    const filePath = Deno.makeTempDirSync() + "/chown_test_file.txt";

    assertThrows(
      () => {
        Deno.chownSync(filePath, uid, gid);
      },
      Deno.errors.NotFound,
      `chown '${filePath}'`,
    );
  },
);

Deno.test(
  {
    permissions: { run: true, write: true },
    ignore: Deno.build.os == "windows",
  },
  async function chownFileNotExist() {
    const { uid, gid } = await getUidAndGid();
    const filePath = (await Deno.makeTempDir()) + "/chown_test_file.txt";

    await assertRejects(
      async () => {
        await Deno.chown(filePath, uid, gid);
      },
      Deno.errors.NotFound,
      `chown '${filePath}'`,
    );
  },
);

Deno.test(
  { permissions: { write: true }, ignore: Deno.build.os == "windows" },
  function chownSyncPermissionDenied() {
    const dirPath = Deno.makeTempDirSync();
    const filePath = dirPath + "/chown_test_file.txt";
    Deno.writeTextFileSync(filePath, "Hello");

    assertThrows(() => {
      // try changing the file's owner to root
      Deno.chownSync(filePath, 0, 0);
    }, Deno.errors.PermissionDenied);
    Deno.removeSync(dirPath, { recursive: true });
  },
);

Deno.test(
  { permissions: { write: true }, ignore: Deno.build.os == "windows" },
  async function chownPermissionDenied() {
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Hello");

    await assertRejects(async () => {
      // try changing the file's owner to root
      await Deno.chown(filePath, 0, 0);
    }, Deno.errors.PermissionDenied);
    await Deno.remove(dirPath, { recursive: true });
  },
);

Deno.test(
  {
    permissions: { run: true, write: true },
    ignore: Deno.build.os == "windows",
  },
  async function chownSyncSucceed() {
    // TODO(bartlomieju): when a file's owner is actually being changed,
    // chown only succeeds if run under privileged user (root)
    // The test script has no such privilege, so need to find a better way to test this case
    const { uid, gid } = await getUidAndGid();

    const dirPath = Deno.makeTempDirSync();
    const filePath = dirPath + "/chown_test_file.txt";
    Deno.writeTextFileSync(filePath, "Hello");

    // the test script creates this file with the same uid and gid,
    // here chown is a noop so it succeeds under non-privileged user
    Deno.chownSync(filePath, uid, gid);

    Deno.removeSync(dirPath, { recursive: true });
  },
);

Deno.test(
  {
    permissions: { run: true, write: true },
    ignore: Deno.build.os == "windows",
  },
  async function chownSyncWithUrl() {
    const { uid, gid } = await getUidAndGid();
    const dirPath = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${dirPath}/chown_test_file.txt`);
    Deno.writeTextFileSync(fileUrl, "Hello");
    Deno.chownSync(fileUrl, uid, gid);
    Deno.removeSync(dirPath, { recursive: true });
  },
);

Deno.test(
  {
    permissions: { run: true, write: true },
    ignore: Deno.build.os == "windows",
  },
  async function chownSucceed() {
    const { uid, gid } = await getUidAndGid();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Hello");
    await Deno.chown(filePath, uid, gid);
    Deno.removeSync(dirPath, { recursive: true });
  },
);

Deno.test(
  {
    permissions: { run: true, write: true },
    ignore: Deno.build.os == "windows",
  },
  async function chownUidOnly() {
    const { uid } = await getUidAndGid();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Foo");
    await Deno.chown(filePath, uid, null);
    Deno.removeSync(dirPath, { recursive: true });
  },
);

Deno.test(
  {
    permissions: { run: true, write: true },
    ignore: Deno.build.os == "windows",
  },
  async function chownWithUrl() {
    // TODO(bartlomieju): same as chownSyncSucceed
    const { uid, gid } = await getUidAndGid();

    const enc = new TextEncoder();
    const dirPath = await Deno.makeTempDir();
    const fileUrl = new URL(`file://${dirPath}/chown_test_file.txt`);
    const fileData = enc.encode("Hello");
    await Deno.writeFile(fileUrl, fileData);

    // the test script creates this file with the same uid and gid,
    // here chown is a noop so it succeeds under non-privileged user
    await Deno.chown(fileUrl, uid, gid);

    Deno.removeSync(dirPath, { recursive: true });
  },
);
