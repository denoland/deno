// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals, assert } from "./test_util.ts";

// chown on Windows is noop for now, so ignore its testing on Windows

async function getUidAndGid(): Promise<{ uid: number; gid: number }> {
  // get the user ID and group ID of the current process
  const uidProc = Deno.run({
    stdout: "piped",
    cmd: ["python", "-c", "import os; print(os.getuid())"],
  });
  const gidProc = Deno.run({
    stdout: "piped",
    cmd: ["python", "-c", "import os; print(os.getgid())"],
  });

  assertEquals((await uidProc.status()).code, 0);
  assertEquals((await gidProc.status()).code, 0);
  const uid = parseInt(new TextDecoder("utf-8").decode(await uidProc.output()));
  uidProc.close();
  const gid = parseInt(new TextDecoder("utf-8").decode(await gidProc.output()));
  gidProc.close();

  return { uid, gid };
}

unitTest(
  { ignore: Deno.build.os == "windows" },
  async function chownNoWritePermission(): Promise<void> {
    const filePath = "chown_test_file.txt";
    try {
      await Deno.chown(filePath, 1000, 1000);
    } catch (e) {
      assert(e instanceof Deno.errors.PermissionDenied);
    }
  },
);

unitTest(
  { perms: { run: true, write: true }, ignore: Deno.build.os == "windows" },
  async function chownSyncFileNotExist(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const filePath = Deno.makeTempDirSync() + "/chown_test_file.txt";

    try {
      Deno.chownSync(filePath, uid, gid);
    } catch (e) {
      assert(e instanceof Deno.errors.NotFound);
    }
  },
);

unitTest(
  { perms: { run: true, write: true }, ignore: Deno.build.os == "windows" },
  async function chownFileNotExist(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const filePath = (await Deno.makeTempDir()) + "/chown_test_file.txt";

    try {
      await Deno.chown(filePath, uid, gid);
    } catch (e) {
      assert(e instanceof Deno.errors.NotFound);
    }
  },
);

unitTest(
  { perms: { write: true }, ignore: Deno.build.os == "windows" },
  function chownSyncPermissionDenied(): void {
    const dirPath = Deno.makeTempDirSync();
    const filePath = dirPath + "/chown_test_file.txt";
    Deno.writeTextFileSync(filePath, "Hello");

    try {
      // try changing the file's owner to root
      Deno.chownSync(filePath, 0, 0);
    } catch (e) {
      assert(e instanceof Deno.errors.PermissionDenied);
    }
    Deno.removeSync(dirPath, { recursive: true });
  },
);

unitTest(
  { perms: { write: true }, ignore: Deno.build.os == "windows" },
  async function chownPermissionDenied(): Promise<void> {
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Hello");

    try {
      // try changing the file's owner to root
      await Deno.chown(filePath, 0, 0);
    } catch (e) {
      assert(e instanceof Deno.errors.PermissionDenied);
    }
    await Deno.remove(dirPath, { recursive: true });
  },
);

unitTest(
  { perms: { run: true, write: true }, ignore: Deno.build.os == "windows" },
  async function chownSyncSucceed(): Promise<void> {
    // TODO: when a file's owner is actually being changed,
    // chown only succeeds if run under priviledged user (root)
    // The test script has no such privilege, so need to find a better way to test this case
    const { uid, gid } = await getUidAndGid();

    const dirPath = Deno.makeTempDirSync();
    const filePath = dirPath + "/chown_test_file.txt";
    Deno.writeTextFileSync(filePath, "Hello");

    // the test script creates this file with the same uid and gid,
    // here chown is a noop so it succeeds under non-priviledged user
    Deno.chownSync(filePath, uid, gid);

    Deno.removeSync(dirPath, { recursive: true });
  },
);

unitTest(
  { perms: { run: true, write: true }, ignore: Deno.build.os == "windows" },
  async function chownSyncWithUrl(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const dirPath = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${dirPath}/chown_test_file.txt`);
    Deno.writeTextFileSync(fileUrl, "Hello");
    Deno.chownSync(fileUrl, uid, gid);
    Deno.removeSync(dirPath, { recursive: true });
  },
);

unitTest(
  { perms: { run: true, write: true }, ignore: Deno.build.os == "windows" },
  async function chownSucceed(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Hello");
    await Deno.chown(filePath, uid, gid);
    Deno.removeSync(dirPath, { recursive: true });
  },
);

unitTest(
  { perms: { run: true, write: true }, ignore: Deno.build.os == "windows" },
  async function chownUidOnly(): Promise<void> {
    const { uid } = await getUidAndGid();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Foo");
    await Deno.chown(filePath, uid, null);
    Deno.removeSync(dirPath, { recursive: true });
  },
);

unitTest(
  { perms: { run: true, write: true }, ignore: Deno.build.os == "windows" },
  async function chownWithUrl(): Promise<void> {
    // TODO: same as chownSyncSucceed
    const { uid, gid } = await getUidAndGid();

    const enc = new TextEncoder();
    const dirPath = await Deno.makeTempDir();
    const fileUrl = new URL(`file://${dirPath}/chown_test_file.txt`);
    const fileData = enc.encode("Hello");
    await Deno.writeFile(fileUrl, fileData);

    // the test script creates this file with the same uid and gid,
    // here chown is a noop so it succeeds under non-priviledged user
    await Deno.chown(fileUrl, uid, gid);

    Deno.removeSync(dirPath, { recursive: true });
  },
);
