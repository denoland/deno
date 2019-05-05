// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEquals } from "./test_util.ts";

// chown on Windows is noop for now, so ignore its testing on Windows
if (Deno.build.os !== "win") {
  // default user ID and group ID of the current process
  const uid = Deno.build.os === "mac" ? 501 : 1000;
  const gid = Deno.build.os === "mac" ? 20 : 1000;

  testPerm({ write: true }, function chownSyncFileNotExist(): void {
    const filePath = Deno.makeTempDirSync() + "/chown_test_file.txt";
    try {
      Deno.chownSync(filePath, uid, gid);
    } catch (e) {
      assertEquals(e.kind, Deno.ErrorKind.NotFound);
      assertEquals(e.name, "NotFound");
    }
  });

  testPerm({ write: true }, async function chownFileNotExist(): Promise<void> {
    const filePath = (await Deno.makeTempDir()) + "/chown_test_file.txt";
    try {
      await Deno.chown(filePath, uid, gid);
    } catch (e) {
      assertEquals(e.kind, Deno.ErrorKind.NotFound);
      assertEquals(e.name, "NotFound");
    }
  });

  testPerm({ write: true }, function chownSyncPermissionDenied(): void {
    const enc = new TextEncoder();
    const dirPath = Deno.makeTempDirSync();
    const filePath = dirPath + "/chown_test_file.txt";
    const fileData = enc.encode("Hello");
    Deno.writeFileSync(filePath, fileData);

    try {
      // try changing the file's owner to root
      Deno.chownSync(filePath, 0, 0);
    } catch (e) {
      assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
      assertEquals(e.name, "PermissionDenied");
    }
    Deno.removeSync(dirPath, { recursive: true });
  });

  testPerm({ write: true }, async function chownPermissionDenied(): Promise<
    void
  > {
    const enc = new TextEncoder();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    const fileData = enc.encode("Hello");
    await Deno.writeFile(filePath, fileData);

    try {
      // try changing the file's owner to root
      await Deno.chown(filePath, 0, 0);
    } catch (e) {
      assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
      assertEquals(e.name, "PermissionDenied");
    }
    await Deno.remove(dirPath, { recursive: true });
  });

  testPerm({ write: true }, function chownSyncSucceed(): void {
    // TODO: when a file's owner is actually being changed,
    // chown only succeeds if run under priviledged user (root)
    // The test script has no such priviledge, so need to find a better way to test this case
    const enc = new TextEncoder();
    const dirPath = Deno.makeTempDirSync();
    const filePath = dirPath + "/chown_test_file.txt";
    const fileData = enc.encode("Hello");
    Deno.writeFileSync(filePath, fileData);

    // the test script creates this file with the same uid and gid,
    // here chown is a noop so it succeeds under non-priviledged user
    Deno.chownSync(filePath, uid, gid);

    Deno.removeSync(dirPath, { recursive: true });
  });

  testPerm({ write: true }, async function chownSucceed(): Promise<void> {
    // TODO: same as chownSyncSucceed
    const enc = new TextEncoder();
    const dirPath = await Deno.makeTempDir();
    const filePath = dirPath + "/chown_test_file.txt";
    const fileData = enc.encode("Hello");
    await Deno.writeFile(filePath, fileData);

    // the test script creates this file with the same uid and gid,
    // here chown is a noop so it succeeds under non-priviledged user
    await Deno.chown(filePath, uid, gid);

    Deno.removeSync(dirPath, { recursive: true });
  });
}
