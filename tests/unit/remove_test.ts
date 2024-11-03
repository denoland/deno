// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertRejects, assertThrows } from "./test_util.ts";

const REMOVE_METHODS = ["remove", "removeSync"] as const;

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeDirSuccess() {
    for (const method of REMOVE_METHODS) {
      // REMOVE EMPTY DIRECTORY
      const path = Deno.makeTempDirSync() + "/subdir";
      Deno.mkdirSync(path);
      const pathInfo = Deno.statSync(path);
      assert(pathInfo.isDirectory); // check exist first
      await Deno[method](path); // remove
      // We then check again after remove
      assertThrows(() => {
        Deno.statSync(path);
      }, Deno.errors.NotFound);
    }
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeFileSuccess() {
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
      assertThrows(() => {
        Deno.statSync(filename);
      }, Deno.errors.NotFound);
    }
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeFileByUrl() {
    for (const method of REMOVE_METHODS) {
      // REMOVE FILE
      const enc = new TextEncoder();
      const data = enc.encode("Hello");

      const tempDir = Deno.makeTempDirSync();
      const fileUrl = new URL(
        `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
      );

      Deno.writeFileSync(fileUrl, data, { mode: 0o666 });
      const fileInfo = Deno.statSync(fileUrl);
      assert(fileInfo.isFile); // check exist first
      await Deno[method](fileUrl); // remove
      // We then check again after remove
      assertThrows(() => {
        Deno.statSync(fileUrl);
      }, Deno.errors.NotFound);
    }
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeFail() {
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

      await assertRejects(
        async () => {
          await Deno[method](path);
        },
        Error,
        `remove '${path}'`,
      );
      // TODO(ry) Is Other really the error we should get here? What would Go do?

      // NON-EXISTENT DIRECTORY/FILE
      await assertRejects(
        async () => {
          await Deno[method]("/baddir");
        },
        Deno.errors.NotFound,
        `remove '/baddir'`,
      );
    }
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeDanglingSymlinkSuccess() {
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
      assertThrows(() => {
        Deno.lstatSync(danglingSymlinkPath);
      }, Deno.errors.NotFound);
    }
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeValidSymlinkSuccess() {
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
      assertThrows(() => {
        Deno.statSync(validSymlinkPath);
      }, Deno.errors.NotFound);
      await Deno[method](filePath);
    }
  },
);

Deno.test({ permissions: { write: false } }, async function removePerm() {
  for (const method of REMOVE_METHODS) {
    await assertRejects(async () => {
      await Deno[method]("/baddir");
    }, Deno.errors.NotCapable);
  }
});

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeAllDirSuccess() {
    for (const method of REMOVE_METHODS) {
      // REMOVE EMPTY DIRECTORY
      let path = Deno.makeTempDirSync() + "/dir/subdir";
      Deno.mkdirSync(path, { recursive: true });
      let pathInfo = Deno.statSync(path);
      assert(pathInfo.isDirectory); // check exist first
      await Deno[method](path, { recursive: true }); // remove
      // We then check again after remove
      assertThrows(
        () => {
          Deno.statSync(path);
        }, // Directory is gone
        Deno.errors.NotFound,
      );

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
      assertThrows(() => {
        Deno.statSync(path);
      }, Deno.errors.NotFound);
      // Directory is gone
    }
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function removeAllFileSuccess() {
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
      assertThrows(() => {
        Deno.statSync(filename);
      }, Deno.errors.NotFound);
      // File is gone
    }
  },
);

Deno.test({ permissions: { write: true } }, async function removeAllFail() {
  for (const method of REMOVE_METHODS) {
    // NON-EXISTENT DIRECTORY/FILE
    await assertRejects(
      async () => {
        // Non-existent
        await Deno[method]("/baddir", { recursive: true });
      },
      Deno.errors.NotFound,
      `remove '/baddir'`,
    );
  }
});

Deno.test({ permissions: { write: false } }, async function removeAllPerm() {
  for (const method of REMOVE_METHODS) {
    await assertRejects(async () => {
      await Deno[method]("/baddir", { recursive: true });
    }, Deno.errors.NotCapable);
  }
});

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { write: true, read: true },
  },
  async function removeUnixSocketSuccess() {
    for (const method of REMOVE_METHODS) {
      // MAKE TEMPORARY UNIX SOCKET
      const path = Deno.makeTempDirSync() + "/test.sock";
      const listener = Deno.listen({ transport: "unix", path });
      listener.close();
      Deno.statSync(path); // check if unix socket exists

      await Deno[method](path);
      assertThrows(() => Deno.statSync(path), Deno.errors.NotFound);
    }
  },
);

if (Deno.build.os === "windows") {
  Deno.test(
    { permissions: { run: true, write: true, read: true } },
    async function removeFileSymlink() {
      const { success } = await new Deno.Command("cmd", {
        args: ["/c", "mklink", "file_link", "bar"],
        stdout: "null",
      }).output();

      assert(success);
      await Deno.remove("file_link");
      await assertRejects(async () => {
        await Deno.lstat("file_link");
      }, Deno.errors.NotFound);
    },
  );

  Deno.test(
    { permissions: { run: true, write: true, read: true } },
    async function removeDirSymlink() {
      const { success } = await new Deno.Command("cmd", {
        args: ["/c", "mklink", "/d", "dir_link", "bar"],
        stdout: "null",
      }).output();

      assert(success);
      await Deno.remove("dir_link");
      await assertRejects(async () => {
        await Deno.lstat("dir_link");
      }, Deno.errors.NotFound);
    },
  );
}
