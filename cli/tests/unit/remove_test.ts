// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

// SYNC

unitTest(
  { perms: { write: true, read: true } },
  function removeSyncDirSuccess(): void {
    // REMOVE EMPTY DIRECTORY
    const path = Deno.makeTempDirSync() + "/subdir";
    Deno.mkdirSync(path);
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    Deno.removeSync(path); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(path);
    } catch (e) {
      err = e;
    }
    // Directory is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  function removeSyncFileSuccess(): void {
    // REMOVE FILE
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile); // check exist first
    Deno.removeSync(filename); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(filename);
    } catch (e) {
      err = e;
    }
    // File is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  function removeSyncFail(): void {
    // NON-EMPTY DIRECTORY
    const path = Deno.makeTempDirSync() + "/dir/subdir";
    const subPath = path + "/subsubdir";
    Deno.mkdirSync(path, { recursive: true });
    Deno.mkdirSync(subPath);
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    const subPathInfo = Deno.statSync(subPath);
    assert(subPathInfo.isDirectory); // check exist first
    let err;
    try {
      // Should not be able to recursively remove
      Deno.removeSync(path);
    } catch (e) {
      err = e;
    }
    // TODO(ry) Is Other really the error we should get here? What would Go do?
    assert(err instanceof Error);
    // NON-EXISTENT DIRECTORY/FILE
    try {
      // Non-existent
      Deno.removeSync("/baddir");
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  function removeSyncDanglingSymlinkSuccess(): void {
    const danglingSymlinkPath = Deno.makeTempDirSync() + "/dangling_symlink";
    if (Deno.build.os === "windows") {
      Deno.symlinkSync("unexistent_file", danglingSymlinkPath, {
        type: "file",
      });
    } else {
      Deno.symlinkSync("unexistent_file", danglingSymlinkPath);
    }
    const pathInfo = Deno.lstatSync(danglingSymlinkPath);
    assert(pathInfo.isSymlink);
    Deno.removeSync(danglingSymlinkPath);
    let err;
    try {
      Deno.lstatSync(danglingSymlinkPath);
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  function removeSyncValidSymlinkSuccess(): void {
    const encoder = new TextEncoder();
    const data = encoder.encode("Test");
    const tempDir = Deno.makeTempDirSync();
    const filePath = tempDir + "/test.txt";
    const validSymlinkPath = tempDir + "/valid_symlink";
    Deno.writeFileSync(filePath, data, { mode: 0o666 });
    if (Deno.build.os === "windows") {
      Deno.symlinkSync(filePath, validSymlinkPath, { type: "file" });
    } else {
      Deno.symlinkSync(filePath, validSymlinkPath);
    }
    const symlinkPathInfo = Deno.statSync(validSymlinkPath);
    assert(symlinkPathInfo.isFile);
    Deno.removeSync(validSymlinkPath);
    let err;
    try {
      Deno.statSync(validSymlinkPath);
    } catch (e) {
      err = e;
    }
    Deno.removeSync(filePath);
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest({ perms: { write: false } }, function removeSyncPerm(): void {
  let err;
  try {
    Deno.removeSync("/baddir");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

unitTest(
  { perms: { write: true, read: true } },
  function removeAllSyncDirSuccess(): void {
    // REMOVE EMPTY DIRECTORY
    let path = Deno.makeTempDirSync() + "/dir/subdir";
    Deno.mkdirSync(path, { recursive: true });
    let pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    Deno.removeSync(path, { recursive: true }); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(path);
    } catch (e) {
      err = e;
    }
    // Directory is gone
    assert(err instanceof Deno.errors.NotFound);

    // REMOVE NON-EMPTY DIRECTORY
    path = Deno.makeTempDirSync() + "/dir/subdir";
    const subPath = path + "/subsubdir";
    Deno.mkdirSync(path, { recursive: true });
    Deno.mkdirSync(subPath);
    pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    const subPathInfo = Deno.statSync(subPath);
    assert(subPathInfo.isDirectory); // check exist first
    Deno.removeSync(path, { recursive: true }); // remove
    // We then check parent directory again after remove
    try {
      Deno.statSync(path);
    } catch (e) {
      err = e;
    }
    // Directory is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  function removeAllSyncFileSuccess(): void {
    // REMOVE FILE
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile); // check exist first
    Deno.removeSync(filename, { recursive: true }); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(filename);
    } catch (e) {
      err = e;
    }
    // File is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest({ perms: { write: true } }, function removeAllSyncFail(): void {
  // NON-EXISTENT DIRECTORY/FILE
  let err;
  try {
    // Non-existent
    Deno.removeSync("/baddir", { recursive: true });
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.NotFound);
});

unitTest({ perms: { write: false } }, function removeAllSyncPerm(): void {
  let err;
  try {
    Deno.removeSync("/baddir", { recursive: true });
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

// ASYNC

unitTest(
  { perms: { write: true, read: true } },
  async function removeDirSuccess(): Promise<void> {
    // REMOVE EMPTY DIRECTORY
    const path = Deno.makeTempDirSync() + "/dir/subdir";
    Deno.mkdirSync(path, { recursive: true });
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    await Deno.remove(path); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(path);
    } catch (e) {
      err = e;
    }
    // Directory is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeFileSuccess(): Promise<void> {
    // REMOVE FILE
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile); // check exist first
    await Deno.remove(filename); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(filename);
    } catch (e) {
      err = e;
    }
    // File is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeFail(): Promise<void> {
    // NON-EMPTY DIRECTORY
    const path = Deno.makeTempDirSync() + "/dir/subdir";
    const subPath = path + "/subsubdir";
    Deno.mkdirSync(path, { recursive: true });
    Deno.mkdirSync(subPath);
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    const subPathInfo = Deno.statSync(subPath);
    assert(subPathInfo.isDirectory); // check exist first
    let err;
    try {
      // Should not be able to recursively remove
      await Deno.remove(path);
    } catch (e) {
      err = e;
    }
    assert(err instanceof Error);
    // NON-EXISTENT DIRECTORY/FILE
    try {
      // Non-existent
      await Deno.remove("/baddir");
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeDanglingSymlinkSuccess(): Promise<void> {
    const danglingSymlinkPath = Deno.makeTempDirSync() + "/dangling_symlink";
    if (Deno.build.os === "windows") {
      Deno.symlinkSync("unexistent_file", danglingSymlinkPath, {
        type: "file",
      });
    } else {
      Deno.symlinkSync("unexistent_file", danglingSymlinkPath);
    }
    const pathInfo = Deno.lstatSync(danglingSymlinkPath);
    assert(pathInfo.isSymlink);
    await Deno.remove(danglingSymlinkPath);
    let err;
    try {
      Deno.lstatSync(danglingSymlinkPath);
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeValidSymlinkSuccess(): Promise<void> {
    const encoder = new TextEncoder();
    const data = encoder.encode("Test");
    const tempDir = Deno.makeTempDirSync();
    const filePath = tempDir + "/test.txt";
    const validSymlinkPath = tempDir + "/valid_symlink";
    Deno.writeFileSync(filePath, data, { mode: 0o666 });
    if (Deno.build.os === "windows") {
      Deno.symlinkSync(filePath, validSymlinkPath, { type: "file" });
    } else {
      Deno.symlinkSync(filePath, validSymlinkPath);
    }
    const symlinkPathInfo = Deno.statSync(validSymlinkPath);
    assert(symlinkPathInfo.isFile);
    await Deno.remove(validSymlinkPath);
    let err;
    try {
      Deno.statSync(validSymlinkPath);
    } catch (e) {
      err = e;
    }
    Deno.removeSync(filePath);
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest({ perms: { write: false } }, async function removePerm(): Promise<
  void
> {
  let err;
  try {
    await Deno.remove("/baddir");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

unitTest(
  { perms: { write: true, read: true } },
  async function removeAllDirSuccess(): Promise<void> {
    // REMOVE EMPTY DIRECTORY
    let path = Deno.makeTempDirSync() + "/dir/subdir";
    Deno.mkdirSync(path, { recursive: true });
    let pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    await Deno.remove(path, { recursive: true }); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(path);
    } catch (e) {
      err = e;
    }
    // Directory is gone
    assert(err instanceof Deno.errors.NotFound);

    // REMOVE NON-EMPTY DIRECTORY
    path = Deno.makeTempDirSync() + "/dir/subdir";
    const subPath = path + "/subsubdir";
    Deno.mkdirSync(path, { recursive: true });
    Deno.mkdirSync(subPath);
    pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory); // check exist first
    const subPathInfo = Deno.statSync(subPath);
    assert(subPathInfo.isDirectory); // check exist first
    await Deno.remove(path, { recursive: true }); // remove
    // We then check parent directory again after remove
    try {
      Deno.statSync(path);
    } catch (e) {
      err = e;
    }
    // Directory is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeAllFileSuccess(): Promise<void> {
    // REMOVE FILE
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile); // check exist first
    await Deno.remove(filename, { recursive: true }); // remove
    // We then check again after remove
    let err;
    try {
      Deno.statSync(filename);
    } catch (e) {
      err = e;
    }
    // File is gone
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest({ perms: { write: true } }, async function removeAllFail(): Promise<
  void
> {
  // NON-EXISTENT DIRECTORY/FILE
  let err;
  try {
    // Non-existent
    await Deno.remove("/baddir", { recursive: true });
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.NotFound);
});

unitTest({ perms: { write: false } }, async function removeAllPerm(): Promise<
  void
> {
  let err;
  try {
    await Deno.remove("/baddir", { recursive: true });
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

if (Deno.build.os === "windows") {
  unitTest(
    { perms: { run: true, write: true, read: true } },
    async function removeFileSymlink(): Promise<void> {
      const symlink = Deno.run({
        cmd: ["cmd", "/c", "mklink", "file_link", "bar"],
        stdout: "null",
      });

      assert(await symlink.status());
      symlink.close();
      await Deno.remove("file_link");
      let err;
      try {
        await Deno.lstat("file_link");
      } catch (e) {
        err = e;
      }
      assert(err instanceof Deno.errors.NotFound);
    }
  );

  unitTest(
    { perms: { run: true, write: true, read: true } },
    async function removeDirSymlink(): Promise<void> {
      const symlink = Deno.run({
        cmd: ["cmd", "/c", "mklink", "/d", "dir_link", "bar"],
        stdout: "null",
      });

      assert(await symlink.status());
      symlink.close();

      await Deno.remove("dir_link");
      let err;
      try {
        await Deno.lstat("dir_link");
      } catch (e) {
        err = e;
      }
      assert(err instanceof Deno.errors.NotFound);
    }
  );
}
