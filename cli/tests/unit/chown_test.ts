// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, assertThrowsAsync } from "./test_util.ts";

// chown on Windows is noop for now, so ignore its testing on Windows

async function getUidAndGid(): Promise<{ uid: number; gid: number }> {
  // get the user ID and group ID of the current process
  const uidProc = Deno.run({
    stdout: "piped",
    cmd: ["id", "-u"],
  });
  const gidProc = Deno.run({
    stdout: "piped",
    cmd: ["id", "-g"],
  });

  assertEquals((await uidProc.status()).code, 0);
  assertEquals((await gidProc.status()).code, 0);
  const uid = parseInt(new TextDecoder("utf-8").decode(await uidProc.output()));
  uidProc.close();
  const gid = parseInt(new TextDecoder("utf-8").decode(await gidProc.output()));
  gidProc.close();

  return { uid, gid };
}

Deno.test({
  name: "chownSyncFileNotExist",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const filePath = Deno.makeTempDirSync() + "/chown_test_file.txt";

    assertThrows(() => {
      Deno.chownSync(filePath, uid, gid);
    }, Deno.errors.NotFound);
  },
});

Deno.test({
  name: "chownFileNotExist",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const filePath = (await Deno.makeTempDir()) + "/chown_test_file.txt";

    await assertThrowsAsync(async () => {
      await Deno.chown(filePath, uid, gid);
    }, Deno.errors.NotFound);
  },
});

Deno.test({
  name: "chownSyncPermissionDenied",
  ignore: Deno.build.os == "windows",
  fn(): void {
    const dirPath = Deno.makeTempDirSync();
    const filePath = dirPath + "/chown_test_file.txt";
    Deno.writeTextFileSync(filePath, "Hello");

    assertThrows(() => {
      // try changing the file's owner to root
      Deno.chownSync(filePath, 0, 0);
    }, Deno.errors.PermissionDenied);
    Deno.removeSync(dirPath, { recursive: true });
  },
});

Deno.test({
  name: "chownPermissionDenied",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Hello");

    await assertThrowsAsync(async () => {
      // try changing the file's owner to root
      await Deno.chown(filePath, 0, 0);
    }, Deno.errors.PermissionDenied);
    await Deno.remove(dirPath, { recursive: true });
  },
});

Deno.test({
  name: "chownSyncSucceed",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    // TODO(bartlomieju): when a file's owner is actually being changed,
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
});

Deno.test({
  name: "chownSyncWithUrl",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const dirPath = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${dirPath}/chown_test_file.txt`);
    Deno.writeTextFileSync(fileUrl, "Hello");
    Deno.chownSync(fileUrl, uid, gid);
    Deno.removeSync(dirPath, { recursive: true });
  },
});

Deno.test({
  name: "chownSucceed",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    const { uid, gid } = await getUidAndGid();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Hello");
    await Deno.chown(filePath, uid, gid);
    Deno.removeSync(dirPath, { recursive: true });
  },
});

Deno.test({
  name: "chownUidOnly",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    const { uid } = await getUidAndGid();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    await Deno.writeTextFile(filePath, "Foo");
    await Deno.chown(filePath, uid, null);
    Deno.removeSync(dirPath, { recursive: true });
  },
});

Deno.test({
  name: "chownWithUrl",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    // TODO(bartlomieju): same as chownSyncSucceed
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
});

Deno.test({
  name: "chownNoWritePermission",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<void> {
    await Deno.permissions.revoke({ name: "write" });

    const filePath = "chown_test_file.txt";
    await assertThrowsAsync(async () => {
      await Deno.chown(filePath, 1000, 1000);
    }, Deno.errors.PermissionDenied);
  },
});
