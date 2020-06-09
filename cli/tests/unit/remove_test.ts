// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

const REMOVE_METHODS = ["remove", "removeSync"] as const;

unitTest(
  { perms: { write: true, read: true } },
  async function removeDirSuccess(): Promise<void> {
    for (const method of REMOVE_METHODS) {
      // REMOVE EMPTY DIRECTORY
      const path = Deno.makeTempDirSync() + "/subdir";
      Deno.mkdirSync(path);
      const pathInfo = Deno.statSync(path);
      assert(pathInfo.isDirectory); // check exist first
      await Deno[method](path); // remove
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
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeFileSuccess(): Promise<void> {
    for (const method of REMOVE_METHODS) {
      // REMOVE FILE
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      Deno.writeFileSync(filename, data, { mode: 0o666 });
      const fileInfo = Deno.statSync(filename);
      assert(fileInfo.isFile); // check exist first
      await Deno[method](filename); // remove
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
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeFileByUrl(): Promise<void> {
    for (const method of REMOVE_METHODS) {
      // REMOVE FILE
      const enc = new TextEncoder();
      const data = enc.encode("Hello");

      const tempDir = Deno.makeTempDirSync();
      const fileUrl = new URL(
        `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`
      );

      Deno.writeFileSync(fileUrl, data, { mode: 0o666 });
      const fileInfo = Deno.statSync(fileUrl);
      assert(fileInfo.isFile); // check exist first
      await Deno[method](fileUrl); // remove
      // We then check again after remove
      let err;
      try {
        Deno.statSync(fileUrl);
      } catch (e) {
        err = e;
      }
      // File is gone
      assert(err instanceof Deno.errors.NotFound);
    }
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeFail(): Promise<void> {
    for (const method of REMOVE_METHODS) {
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
        await Deno[method](path);
      } catch (e) {
        err = e;
      }
      // TODO(ry) Is Other really the error we should get here? What would Go do?
      assert(err instanceof Error);
      // NON-EXISTENT DIRECTORY/FILE
      try {
        // Non-existent
        await Deno[method]("/baddir");
      } catch (e) {
        err = e;
      }
      assert(err instanceof Deno.errors.NotFound);
    }
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeDanglingSymlinkSuccess(): Promise<void> {
    for (const method of REMOVE_METHODS) {
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
      await Deno[method](danglingSymlinkPath);
      let err;
      try {
        Deno.lstatSync(danglingSymlinkPath);
      } catch (e) {
        err = e;
      }
      assert(err instanceof Deno.errors.NotFound);
    }
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeValidSymlinkSuccess(): Promise<void> {
    for (const method of REMOVE_METHODS) {
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
      await Deno[method](validSymlinkPath);
      let err;
      try {
        Deno.statSync(validSymlinkPath);
      } catch (e) {
        err = e;
      }
      await Deno[method](filePath);
      assert(err instanceof Deno.errors.NotFound);
    }
  }
);

unitTest({ perms: { write: false } }, async function removePerm(): Promise<
  void
> {
  for (const method of REMOVE_METHODS) {
    let err;
    try {
      await Deno[method]("/baddir");
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
});

unitTest(
  { perms: { write: true, read: true } },
  async function removeAllDirSuccess(): Promise<void> {
    for (const method of REMOVE_METHODS) {
      // REMOVE EMPTY DIRECTORY
      let path = Deno.makeTempDirSync() + "/dir/subdir";
      Deno.mkdirSync(path, { recursive: true });
      let pathInfo = Deno.statSync(path);
      assert(pathInfo.isDirectory); // check exist first
      await Deno[method](path, { recursive: true }); // remove
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
      await Deno[method](path, { recursive: true }); // remove
      // We then check parent directory again after remove
      try {
        Deno.statSync(path);
      } catch (e) {
        err = e;
      }
      // Directory is gone
      assert(err instanceof Deno.errors.NotFound);
    }
  }
);

unitTest(
  { perms: { write: true, read: true } },
  async function removeAllFileSuccess(): Promise<void> {
    for (const method of REMOVE_METHODS) {
      // REMOVE FILE
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      Deno.writeFileSync(filename, data, { mode: 0o666 });
      const fileInfo = Deno.statSync(filename);
      assert(fileInfo.isFile); // check exist first
      await Deno[method](filename, { recursive: true }); // remove
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
  }
);

unitTest({ perms: { write: true } }, async function removeAllFail(): Promise<
  void
> {
  for (const method of REMOVE_METHODS) {
    // NON-EXISTENT DIRECTORY/FILE
    let err;
    try {
      // Non-existent
      await Deno[method]("/baddir", { recursive: true });
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.NotFound);
  }
});

unitTest({ perms: { write: false } }, async function removeAllPerm(): Promise<
  void
> {
  for (const method of REMOVE_METHODS) {
    let err;
    try {
      await Deno[method]("/baddir", { recursive: true });
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
});

unitTest(
  {
    ignore: Deno.build.os === "windows",
    perms: { write: true, read: true },
  },
  async function removeUnixSocketSuccess(): Promise<void> {
    for (const method of REMOVE_METHODS) {
      // MAKE TEMPORARY UNIX SOCKET
      const path = Deno.makeTempDirSync() + "/test.sock";
      const listener = Deno.listen({ transport: "unix", path });
      listener.close();
      Deno.statSync(path); // check if unix socket exists

      await Deno[method](path);
      let err;
      try {
        Deno.statSync(path);
      } catch (e) {
        err = e;
      }
      assert(err instanceof Deno.errors.NotFound);
    }
  }
);

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
