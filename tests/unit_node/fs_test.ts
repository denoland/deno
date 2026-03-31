// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference lib="deno.ns" />
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  fail,
} from "@std/assert";
import { Buffer } from "node:buffer";
import { join } from "node:path";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import {
  accessSync,
  appendFile,
  appendFileSync,
  chmod,
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  copyFileSync,
  createWriteStream,
  existsSync,
  fchmod,
  fchmodSync,
  fchown,
  fchownSync,
  fdatasync,
  fdatasyncSync,
  fsync,
  fsyncSync,
  ftruncate,
  ftruncateSync,
  futimes,
  futimesSync,
  link,
  linkSync,
  lstatSync,
  mkdtempSync,
  openAsBlob,
  opendirSync,
  openSync,
  promises,
  promises as fsPromises,
  readFileSync,
  readSync,
  Stats,
  statSync,
  unlink,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { readFile } from "node:fs/promises";
import {
  constants as fsPromiseConstants,
  copyFile,
  cp,
  FileHandle,
  lchown,
  lutimes,
  open,
  stat,
  statfs,
  writeFile,
} from "node:fs/promises";
import { fromFileUrl } from "@std/path";
import process from "node:process";
import { setTimeout as setTimeoutPromise } from "node:timers/promises";
import { assertCallbackErrorUncaught } from "./_test_utils.ts";
import { pathToAbsoluteFileUrl } from "../unit/test_util.ts";

Deno.test(
  "[node/fs writeFileSync] write file without option",
  () => {
    const data = "Hello";
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";

    writeFileSync(filename, data);
    const dataRead = readFileSync(filename, "utf8");

    assert(dataRead === "Hello");
  },
);

Deno.test(
  "[node/fs writeFileSync] write file with option ASCII",
  () => {
    const data = "Hello";
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";

    writeFileSync(filename, data, { encoding: "ascii" });
    const dataRead = readFileSync(filename, "utf8");

    assert(dataRead === "Hello");
  },
);

Deno.test(
  "[node/fs existsSync] path",
  { permissions: { read: true } },
  () => {
    assert(existsSync("tests/testdata/assets/fixture.json"));
  },
);

Deno.test(
  "[node/fs existsSync] url",
  { permissions: { read: true } },
  () => {
    assert(existsSync(
      pathToAbsoluteFileUrl("tests/testdata/assets/fixture.json"),
    ));
  },
);

Deno.test(
  "[node/fs existsSync] no permission",
  { permissions: { read: false } },
  () => {
    assertThrows(() => {
      existsSync("tests/testdata/assets/fixture.json");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  "[node/fs existsSync] not exists",
  { permissions: { read: true } },
  () => {
    assert(!existsSync("bad_filename"));
  },
);

Deno.test(
  "[node/fs/promises constants] is the same as from node:fs",
  () => {
    assertEquals(constants, fsPromiseConstants);
    assertEquals(constants, promises.constants);
  },
);

Deno.test(
  "[node/fs statSync] instanceof fs.Stats",
  () => {
    const stat = statSync("tests/testdata/assets/fixture.json");
    assert(stat);
    assert(stat instanceof Stats);
  },
);

Deno.test(
  "[node/fs statSync] throw error with path information",
  () => {
    const file = "non-exist-file";
    const fileUrl = new URL(file, import.meta.url);

    assertThrows(() => {
      statSync(file);
    }, "Error: ENOENT: no such file or directory, stat 'non-exist-file'");

    assertThrows(() => {
      statSync(fileUrl);
    }, `Error: ENOENT: no such file or directory, stat '${fileUrl.pathname}'`);
  },
);

Deno.test(
  "[node/fs/promises stat] throw error with path information",
  async () => {
    const file = "non-exist-file";
    const fileUrl = new URL(file, import.meta.url);

    try {
      await stat(file);
    } catch (error: unknown) {
      assertEquals(
        `${error}`,
        "Error: ENOENT: no such file or directory, stat 'non-exist-file'",
      );
    }

    try {
      await stat(fileUrl);
    } catch (error: unknown) {
      assertEquals(
        `${error}`,
        `Error: ENOENT: no such file or directory, stat '${
          fileURLToPath(fileUrl)
        }'`,
      );
    }
  },
);

Deno.test(
  "[node/fs/promises statfs] export statfs function",
  async () => {
    await statfs(import.meta.filename!);
  },
);

Deno.test(
  "[node/fs/promises cp] copy file",
  async () => {
    const src = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";
    const dest = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";
    writeFileSync(src, "Hello");

    await cp(src, dest);

    const dataRead = readFileSync(dest, "utf8");
    assert(dataRead === "Hello");
  },
);

// TODO(kt3k): Delete this test case, and instead enable the compat case
// `test/parallel/test-fs-writestream-open-write.js`, when we update
// `tests/node_compat/runner/suite`.
Deno.test("[node/fs createWriteStream", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const tempDir = await Deno.makeTempDir();
  const file = join(tempDir, "file.txt");
  try {
    const w = createWriteStream(file);

    w.on("open", () => {
      w.write("hello, ");

      process.nextTick(() => {
        w.write("world");
        w.end();
      });
    });

    w.on("close", async () => {
      try {
        assertEquals(await Deno.readTextFile(file), "hello, world");
        resolve();
      } catch (e) {
        reject(e);
      }
    });
    await promise;
  } finally {
    await Deno.remove(tempDir, { recursive: true });
  }
});

Deno.test("[node/fs] FileHandle.appendFile", async () => {
  const tempDir = await Deno.makeTempDir();
  const filePath = join(tempDir, "test_append.txt");
  const initialContent = "Hello, ";
  const appendContent = "World!";
  const expectedContent = "Hello, World!";

  try {
    await Deno.writeTextFile(filePath, initialContent);
    const fileHandle = await fsPromises.open(filePath, "a+");
    try {
      await fileHandle.appendFile(appendContent);
      const content = await Deno.readTextFile(filePath);
      assertEquals(content, expectedContent);
      const binaryData = new Uint8Array([65, 66, 67]);
      await fileHandle.appendFile(binaryData);
      const finalContent = await Deno.readFile(filePath);
      const expectedBinary = new TextEncoder().encode(expectedContent);
      const expectedFinal = new Uint8Array([...expectedBinary, ...binaryData]);
      assertEquals(finalContent, expectedFinal);
    } finally {
      await fileHandle.close().catch(() => {});
    }
  } finally {
    await Deno.remove(tempDir, { recursive: true }).catch(() => {});
  }
});

Deno.test(
  "[node/fs lstatSync] supports throwIfNoEntry option",
  () => {
    const result = lstatSync("non-existing-path", { throwIfNoEntry: false });
    assertEquals(result, undefined);
  },
);

// Test for https://github.com/denoland/deno/issues/23707
Deno.test(
  "[node/fs/promises read] respect position argument",
  async () => {
    const file = mkdtempSync(join(tmpdir(), "foo-")) + "/test.bin";
    await writeFile(file, "");

    const res: number[] = [];
    let fd: FileHandle | undefined;
    try {
      fd = await open(file, "r+");

      for (let i = 0; i <= 5; i++) {
        const buffer = new Uint8Array([i]);
        await fd.write(buffer, 0, 1, i + 10);
      }

      for (let i = 10; i <= 15; i++) {
        const buffer = new Uint8Array(1);
        await fd.read(buffer, 0, 1, i);
        res.push(Number(buffer.toString()));
      }
    } finally {
      await fd?.close();
    }

    assertEquals(res, [0, 1, 2, 3, 4, 5]);
  },
);

Deno.test("[node/fs] readSync works", () => {
  const fd = openSync("tests/testdata/assets/hello.txt", "r");
  const buf = new Uint8Array(256);
  const bytesRead = readSync(fd!, buf);
  assertEquals(bytesRead, 12);
  closeSync(fd!);
});

Deno.test("[node/fs] copyFile COPYFILE_EXCL works", async () => {
  const dir = mkdtempSync(join(tmpdir(), "foo-"));
  const src = join(dir, "src.txt");
  const dest = join(dir, "dest.txt");
  await writeFile(src, "");
  await copyFile(src, dest, fsPromiseConstants.COPYFILE_EXCL);
  assert(existsSync(dest));
  await assertRejects(() =>
    copyFile(src, dest, fsPromiseConstants.COPYFILE_EXCL)
  );
  const dest2 = join(dir, "dest2.txt");
  copyFileSync(src, dest2, fsPromiseConstants.COPYFILE_EXCL);
  assert(existsSync(dest2));
  assertThrows(() =>
    copyFileSync(src, dest2, fsPromiseConstants.COPYFILE_EXCL)
  );
});

Deno.test("[node/fs] statSync throws ENOENT for invalid path containing colon in it", () => {
  // deno-lint-ignore no-explicit-any
  const err: any = assertThrows(() => {
    // Note: Deno.stat throws ERROR_INVALID_NAME (os error 123) instead of
    // ERROR_FILE_NOT_FOUND (os error 2) on windows. This case checks that
    // ERROR_INVALID_NAME is mapped to ENOENT correctly on node compat layer.
    statSync("jsr:@std/assert");
  });
  assertEquals(err.code, "ENOENT");
});

Deno.test("[node/fs] readFile aborted with signal", async () => {
  const src = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";
  await writeFile(src, "Hello");
  const signal = AbortSignal.abort();
  await assertRejects(
    () => readFile(src, { signal }),
    DOMException,
    "The signal has been aborted",
  );
});

async function execCmd(cmd: string) {
  const dec = new TextDecoder();
  const [bin, ...args] = cmd.split(" ");
  const command = new Deno.Command(bin, { args });
  const { code, stdout, stderr } = await command.output();
  if (code !== 0) {
    throw new Error(
      `Command failed with code ${code}: ${cmd} - ${dec.decode(stderr)}`,
    );
  }
  return dec.decode(stdout).trim();
}

Deno.test("[node/fs] fchown and fchownSync", {
  ignore: Deno.build.os === "windows",
}, async () => {
  const file = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";
  await writeFile(file, "Hello");
  const uid = await execCmd("id -u");
  const gid = await execCmd("id -g");
  const fd = openSync(file, "r+");
  // Changing the owner of a file to the current user is not an error.
  await new Promise<void>((resolve) =>
    fchown(fd, +uid, +gid, (err) => {
      assertEquals(err, null);
      resolve();
    })
  );
  fchownSync(fd, +uid, +gid);
  // Changing the owner of a file to root is an error.
  await assertRejects(() =>
    new Promise<void>((resolve, reject) =>
      fchown(fd, 0, 0, (err) => {
        if (err) {
          reject(err);
        } else {
          resolve();
        }
      })
    )
  );
  assertThrows(() => {
    fchownSync(fd, 0, 0);
  });
  closeSync(fd);
});

Deno.test("[node/fs] fchmod works", {
  ignore: Deno.build.os === "windows",
}, async () => {
  // Prepare
  const tempFile = await Deno.makeTempFile();
  const originalFileMode = (await Deno.lstat(tempFile)).mode;
  const fd = openSync(tempFile, "r+");
  // Execute
  await new Promise<void>((resolve, reject) => {
    fchmod(fd, 0o777, (err) => {
      if (err) {
        reject(err);
      } else {
        resolve();
      }
    });
  })
    // Assert
    .then(() => {
      const newFileMode = Deno.lstatSync(tempFile).mode;
      assert(newFileMode && originalFileMode);
      assert(newFileMode === 33279 && newFileMode > originalFileMode);
    }, (error) => {
      fail(error);
    })
    .finally(() => {
      closeSync(fd);
      Deno.removeSync(tempFile);
    });
});

Deno.test("[node/fs] fchmodSync works", {
  ignore: Deno.build.os === "windows",
}, () => {
  // Prepare
  const tempFile = Deno.makeTempFileSync();
  const originalFileMode = Deno.lstatSync(tempFile).mode;
  const fd = openSync(tempFile, "r+");
  // Execute
  fchmodSync(fd, 0o777);
  // Assert
  const newFileMode = Deno.lstatSync(tempFile).mode;
  assert(newFileMode && originalFileMode);
  assert(newFileMode === 33279 && newFileMode > originalFileMode);
  closeSync(fd);
  Deno.removeSync(tempFile);
});

Deno.test("[node/fs/promises] lchown works", {
  ignore: Deno.build.os === "windows",
}, async () => {
  const tempFile = Deno.makeTempFileSync();
  const symlinkPath = tempFile + "-link";
  Deno.symlinkSync(tempFile, symlinkPath);
  const uid = await execCmd("id -u");
  const gid = await execCmd("id -g");

  await lchown(symlinkPath, +uid, +gid);

  Deno.removeSync(tempFile);
  Deno.removeSync(symlinkPath);
});

Deno.test("[node/fs/promises] lutimes works", {
  ignore: Deno.build.os === "windows",
}, async () => {
  const tempFile = Deno.makeTempFileSync();
  const symlinkPath = tempFile + "-link";
  Deno.symlinkSync(tempFile, symlinkPath);

  const date = new Date("1970-01-01T00:00:00Z");
  await lutimes(symlinkPath, date, date);

  const stats = Deno.lstatSync(symlinkPath);
  assertEquals((stats.atime as Date).getTime(), date.getTime());
  assertEquals((stats.mtime as Date).getTime(), date.getTime());

  Deno.removeSync(tempFile);
  Deno.removeSync(symlinkPath);
});

Deno.test("[node/fs] constants are correct across platforms", () => {
  assert(constants.R_OK === 4);
  // Check a handful of constants with different values across platforms
  if (Deno.build.os === "darwin") {
    assert(constants.UV_FS_O_FILEMAP === 0);
    assert(constants.O_CREAT === 0x200);
    assert(constants.O_DIRECT === undefined);
    assert(constants.O_NOATIME === undefined);
    assert(constants.O_SYMLINK === 0x200000);
  }
  if (Deno.build.os === "linux") {
    assert(constants.UV_FS_O_FILEMAP === 0);
    assert(constants.O_CREAT === 0x40);
    assert(constants.O_DIRECT !== undefined); // O_DIRECT has different values between architectures
    assert(constants.O_NOATIME === 0x40000);
    assert(constants.O_SYMLINK === undefined);
  }
  if (Deno.build.os === "windows") {
    assert(constants.UV_FS_O_FILEMAP === 0x20000000);
    assert(constants.O_CREAT === 0x100);
    assert(constants.O_DIRECT === undefined);
    assert(constants.O_NOATIME === undefined);
    assert(constants.O_SYMLINK === undefined);
  }
});

Deno.test(
  "[node/fs openAsBlob] returns a Blob with file contents",
  async () => {
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";
    const data = "Hello, openAsBlob!";
    writeFileSync(filename, data);

    const blob = await openAsBlob(filename);
    assertEquals(blob instanceof Blob, true);
    assertEquals(await blob.text(), data);
    assertEquals(blob.type, "");
  },
);

Deno.test(
  "[node/fs openAsBlob] respects type option",
  async () => {
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";
    writeFileSync(filename, "content");

    const blob = await openAsBlob(filename, { type: "text/plain" });
    assertEquals(blob.type, "text/plain");
    assertEquals(await blob.text(), "content");
  },
);

Deno.test(
  "[node/fs openAsBlob] rejects for non-existent file",
  async () => {
    await assertRejects(
      () => openAsBlob("/non/existent/file.txt"),
    );
  },
);

Deno.test(
  "[node/fs.access] Uses the owner permission when the user is the owner",
  { ignore: Deno.build.os === "windows" },
  async () => {
    const file = await Deno.makeTempFile();
    try {
      await Deno.chmod(file, 0o600);
      await promises.access(file, constants.R_OK);
      await promises.access(file, constants.W_OK);
      await assertRejects(async () => {
        await promises.access(file, constants.X_OK);
      });
    } finally {
      await Deno.remove(file);
    }
  },
);

Deno.test(
  "[node/fs.access] doesn't reject on windows",
  { ignore: Deno.build.os !== "windows" },
  async () => {
    const file = await Deno.makeTempFile();
    try {
      await promises.access(file, constants.R_OK);
      await promises.access(file, constants.W_OK);
      await promises.access(file, constants.X_OK);
      await promises.access(file, constants.F_OK);
    } finally {
      await Deno.remove(file);
    }
  },
);

Deno.test(
  "[node/fs.accessSync] Uses the owner permission when the user is the owner",
  { ignore: Deno.build.os === "windows" },
  () => {
    const file = Deno.makeTempFileSync();
    try {
      Deno.chmodSync(file, 0o600);
      accessSync(file, constants.R_OK);
      accessSync(file, constants.W_OK);
      assertThrows(() => {
        accessSync(file, constants.X_OK);
      });
    } finally {
      Deno.removeSync(file);
    }
  },
);

Deno.test(
  "[node/fs.accessSync] doesn't throw on windows",
  { ignore: Deno.build.os !== "windows" },
  () => {
    const file = Deno.makeTempFileSync();
    try {
      accessSync(file, constants.R_OK);
      accessSync(file, constants.W_OK);
      accessSync(file, constants.X_OK);
      accessSync(file, constants.F_OK);
    } finally {
      Deno.removeSync(file);
    }
  },
);

// ==========
// appendFile tests (from _fs_appendFile_test.ts)
// ==========

const appendFileDecoder = new TextDecoder("utf-8");

Deno.test({
  name: "[node/fs.appendFile] No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Argument of type 'string' is not assignable to parameter of type 'NoParamCallback'
        appendFile("some/path", "some data", "utf8");
      },
      Error,
      "The \"cb\" argument must be of type function. Received type string ('utf8')",
    );
  },
});

Deno.test({
  name: "[node/fs.appendFile] Unsupported encoding results in error()",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
        appendFile("some/path", "some data", "made-up-encoding", () => {});
      },
      Error,
      "The argument 'encoding' is invalid encoding. Received 'made-up-encoding'",
    );
    assertThrows(
      () => {
        appendFile(
          "some/path",
          "some data",
          // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
          { encoding: "made-up-encoding" },
          () => {},
        );
      },
      Error,
      "The argument 'encoding' is invalid encoding. Received 'made-up-encoding'",
    );
    assertThrows(
      // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
      () => appendFileSync("some/path", "some data", "made-up-encoding"),
      Error,
      "The argument 'encoding' is invalid encoding. Received 'made-up-encoding'",
    );
    assertThrows(
      () =>
        appendFileSync("some/path", "some data", {
          // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
          encoding: "made-up-encoding",
        }),
      Error,
      "The argument 'encoding' is invalid encoding. Received 'made-up-encoding'",
    );
  },
});

Deno.test({
  name: "[node/fs.appendFile] Async: Data is written to passed in rid",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    using file = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      appendFile(file.rid, "hello world", (err) => {
        if (err) reject();
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile(tempFile);
        assertEquals(appendFileDecoder.decode(data), "hello world");
      }, () => {
        fail("No error expected");
      })
      .finally(async () => {
        await Deno.remove(tempFile);
      });
  },
});

Deno.test({
  name: "[node/fs.appendFile] Async: Data is written to passed in file path",
  async fn() {
    await new Promise<void>((resolve, reject) => {
      appendFile("_fs_appendFile_test_file.txt", "hello world", (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile("_fs_appendFile_test_file.txt");
        assertEquals(appendFileDecoder.decode(data), "hello world");
      }, (err) => {
        fail("No error was expected: " + err);
      })
      .finally(async () => {
        await Deno.remove("_fs_appendFile_test_file.txt");
      });
  },
});

Deno.test({
  name: "[node/fs.appendFile] Async: Data is written to passed in URL",
  async fn() {
    const fileURL = new URL("_fs_appendFile_test_file.txt", import.meta.url);
    await new Promise<void>((resolve, reject) => {
      appendFile(fileURL, "hello world", (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile(fromFileUrl(fileURL));
        assertEquals(appendFileDecoder.decode(data), "hello world");
      }, (err) => {
        fail("No error was expected: " + err);
      })
      .finally(async () => {
        await Deno.remove(fromFileUrl(fileURL));
      });
  },
});

Deno.test({
  name:
    "[node/fs.appendFile] Async: Callback is made with error if attempting to append data to an existing file with 'ax' flag",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await new Promise<void>((resolve, reject) => {
      appendFile(tempFile, "hello world", { flag: "ax" }, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(() => fail("Expected error to be thrown"))
      .catch(() => {})
      .finally(async () => {
        await Deno.remove(tempFile);
      });
  },
});

Deno.test({
  name: "[node/fs.appendFileSync] Sync: Data is written to passed in rid",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    using file = Deno.openSync(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
    appendFileSync(file.rid, "hello world");
    const data = Deno.readFileSync(tempFile);
    assertEquals(appendFileDecoder.decode(data), "hello world");
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "[node/fs.appendFileSync] Sync: Data is written to passed in file path",
  fn() {
    appendFileSync("_fs_appendFile_test_file_sync.txt", "hello world");
    const data = Deno.readFileSync("_fs_appendFile_test_file_sync.txt");
    assertEquals(appendFileDecoder.decode(data), "hello world");
    Deno.removeSync("_fs_appendFile_test_file_sync.txt");
  },
});

Deno.test({
  name:
    "[node/fs.appendFileSync] Sync: error thrown if attempting to append data to an existing file with 'ax' flag",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    assertThrows(
      () => appendFileSync(tempFile, "hello world", { flag: "ax" }),
      Error,
      "",
    );
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name:
    "[node/fs.appendFileSync] Sync: Data is written in Uint8Array to passed in file path",
  fn() {
    const testData = new TextEncoder().encode("hello world");
    appendFileSync("_fs_appendFile_test_file_sync.txt", testData);
    const data = Deno.readFileSync("_fs_appendFile_test_file_sync.txt");
    assertEquals(data, testData);
    Deno.removeSync("_fs_appendFile_test_file_sync.txt");
  },
});

Deno.test({
  name:
    "[node/fs.appendFile] Async: Data is written in Uint8Array to passed in file path",
  async fn() {
    const testData = new TextEncoder().encode("hello world");
    await new Promise<void>((resolve, reject) => {
      appendFile("_fs_appendFile_test_file.txt", testData, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile("_fs_appendFile_test_file.txt");
        assertEquals(data, testData);
      }, (err) => {
        fail("No error was expected: " + err);
      })
      .finally(async () => {
        await Deno.remove("_fs_appendFile_test_file.txt");
      });
  },
});

Deno.test("[node/fs.appendFile] appendFile callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { appendFile } from ${JSON.stringify(importUrl)}`,
    invocation: `appendFile(${JSON.stringify(tempFile)}, "hello world", `,
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});

// ==========
// chmod tests (from _fs_chmod_test.ts)
// ==========

let chmodModeAsync: number;
let chmodModeSync: number;
// On Windows chmod is only able to manipulate write permission
if (Deno.build.os === "windows") {
  chmodModeAsync = 0o444; // read-only
  chmodModeSync = 0o666; // read-write
} else {
  chmodModeAsync = 0o777;
  chmodModeSync = 0o644;
}

Deno.test({
  name: "[node/fs.chmod] ASYNC: Permissions are changed",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await new Promise<void>((resolve, reject) => {
      chmod(tempFile, chmodModeAsync, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(() => {
        const fileMode = Deno.lstatSync(tempFile).mode as number;
        assertEquals(fileMode & 0o777, chmodModeAsync);
      }, (error) => {
        fail(error);
      })
      .finally(() => {
        Deno.removeSync(tempFile);
      });
  },
});

Deno.test({
  name: "[node/fs.chmod] ASYNC: don't swallow NotFoundError (Windows)",
  ignore: Deno.build.os !== "windows",
  async fn() {
    await assertRejects(async () => {
      await new Promise<void>((resolve, reject) => {
        chmod("./__non_existent_file__", 0o777, (err) => {
          if (err) reject(err);
          else resolve();
        });
      });
    });
  },
});

Deno.test({
  name: "[node/fs.chmodSync] SYNC: Permissions are changed",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    try {
      chmodSync(tempFile, chmodModeSync.toString(8));

      const fileMode = Deno.lstatSync(tempFile).mode as number;
      assertEquals(fileMode & 0o777, chmodModeSync);
    } finally {
      Deno.removeSync(tempFile);
    }
  },
});

Deno.test({
  name: "[node/fs.chmodSync] SYNC: don't swallow NotFoundError (Windows)",
  ignore: Deno.build.os !== "windows",
  fn() {
    assertThrows(() => {
      chmodSync("./__non_existent_file__", "777");
    });
  },
});

Deno.test({
  name: "[node/fs.chmod] chmod callback isn't called twice if error is thrown",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    const importUrl = new URL("node:fs", import.meta.url);
    await assertCallbackErrorUncaught({
      prelude: `import { chmod } from ${JSON.stringify(importUrl)}`,
      invocation: `chmod(${JSON.stringify(tempFile)}, 0o777, `,
      async cleanup() {
        await Deno.remove(tempFile);
      },
    });
  },
});

// ==========
// chown tests (from _fs_chown_test.ts)
// ==========

const chownIgnore = Deno.build.os === "windows";

Deno.test({
  ignore: chownIgnore,
  name:
    "[node/fs.chown] ASYNC: setting existing uid/gid works as expected (non-Windows)",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const originalUserId: number | null = (await Deno.lstat(tempFile)).uid;
    const originalGroupId: number | null = (await Deno.lstat(tempFile)).gid;
    await new Promise<void>((resolve, reject) => {
      chown(tempFile, originalUserId!, originalGroupId!, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(() => {
        const newUserId: number | null = Deno.lstatSync(tempFile).uid;
        const newGroupId: number | null = Deno.lstatSync(tempFile).gid;
        assertEquals(newUserId, originalUserId);
        assertEquals(newGroupId, originalGroupId);
      }, () => {
        fail();
      })
      .finally(() => {
        Deno.removeSync(tempFile);
      });
  },
});

Deno.test({
  ignore: chownIgnore,
  name:
    "[node/fs.chownSync] SYNC: setting existing uid/gid works as expected (non-Windows)",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const originalUserId: number | null = Deno.lstatSync(tempFile).uid;
    const originalGroupId: number | null = Deno.lstatSync(tempFile).gid;
    chownSync(tempFile, originalUserId!, originalGroupId!);

    const newUserId: number | null = Deno.lstatSync(tempFile).uid;
    const newGroupId: number | null = Deno.lstatSync(tempFile).gid;
    assertEquals(newUserId, originalUserId);
    assertEquals(newGroupId, originalGroupId);
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "[node/fs.chown] chown callback isn't called twice if error is thrown",
  ignore: Deno.build.os === "windows",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    const { uid, gid } = await Deno.lstat(tempFile);
    const importUrl = new URL("node:fs", import.meta.url);
    await assertCallbackErrorUncaught({
      prelude: `import { chown } from ${JSON.stringify(importUrl)}`,
      invocation: `chown(${JSON.stringify(tempFile)}, ${uid}, ${gid}, `,
      async cleanup() {
        await Deno.remove(tempFile);
      },
    });
  },
});

// ==========
// close tests (from _fs_close_test.ts)
// ==========

Deno.test({
  name: "[node/fs.close] ASYNC: File is closed",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const file: Deno.FsFile = await Deno.open(tempFile);

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      close(file.rid, (err) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .catch(() => fail("No error expected"))
      .finally(async () => {
        await Deno.remove(tempFile);
      });
  },
});

Deno.test({
  name: "[node/fs.close] ASYNC: Invalid fd",
  fn() {
    assertThrows(() => {
      close(-1, (_err) => {});
    }, RangeError);
  },
});

Deno.test({
  name: "[node/fs.close] close callback should be asynchronous",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const file: Deno.FsFile = Deno.openSync(tempFile);

    let foo: string;
    const promise = new Promise<void>((resolve) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      close(file.rid, () => {
        assert(foo === "bar");
        resolve();
      });
      foo = "bar";
    });

    await promise;
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "[node/fs.closeSync] SYNC: File is closed",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const file: Deno.FsFile = Deno.openSync(tempFile);

    // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
    closeSync(file.rid);
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "[node/fs.closeSync] SYNC: Invalid fd",
  fn() {
    assertThrows(() => closeSync(-1));
  },
});

Deno.test({
  name: "[node/fs.close] close callback isn't called twice if error is thrown",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
}, async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `
    import { close } from ${JSON.stringify(importUrl)};

    const file = await Deno.open(${JSON.stringify(tempFile)});
    `,
    invocation: "close(file.rid, ",
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});

Deno.test({
  name: "[node/fs.close] close with default callback if none is provided",
}, async () => {
  const tempFile = await Deno.makeTempFile();
  const rid = openSync(tempFile, "r");
  close(rid);
  await setTimeoutPromise(1000);
  assertThrows(() => {
    closeSync(rid), Deno.errors.BadResource;
  });
  await Deno.remove(tempFile);
});

// ==========
// fsync tests (from _fs_fsync_test.ts)
// ==========

Deno.test({
  name:
    "[node/fs.fsync] ASYNC: flush any pending data of the given file stream to disk",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = await Deno.makeTempFile();
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    await file.truncate(size);

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      fsync(file.rid, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        async () => {
          assertEquals((await Deno.stat(filePath)).size, size);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(async () => {
        await Deno.remove(filePath);
      });
  },
});

Deno.test({
  name:
    "[node/fs.fsyncSync] SYNC: flush any pending data the given file stream to disk",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    file.truncateSync(size);

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      fsyncSync(file.rid);
      assertEquals(Deno.statSync(filePath).size, size);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

// ==========
// fdatasync tests (from _fs_fdatasync_test.ts)
// ==========

Deno.test({
  name:
    "[node/fs.fdatasync] ASYNC: flush any pending data operations of the given file stream to disk",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = await Deno.makeTempFile();
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    await file.write(data);

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      fdatasync(file.rid, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        async () => {
          assertEquals(await Deno.readFile(filePath), data);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(async () => {
        await Deno.remove(filePath);
      });
  },
});

Deno.test({
  name:
    "[node/fs.fdatasyncSync] SYNC: flush any pending data operations of the given file stream to disk.",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    file.writeSync(data);

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      fdatasyncSync(file.rid);
      assertEquals(Deno.readFileSync(filePath), data);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

// ==========
// link tests (from _fs_link_test.ts)
// ==========

Deno.test({
  name: "[node/fs.link] ASYNC: hard linking files works as expected",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const linkedFile: string = tempFile + ".link";
    await new Promise<void>((res, rej) => {
      link(tempFile, linkedFile, (err) => {
        if (err) rej(err);
        else res();
      });
    })
      .then(() => {
        assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));
      }, () => {
        fail("Expected to succeed");
      })
      .finally(() => {
        Deno.removeSync(tempFile);
        Deno.removeSync(linkedFile);
      });
  },
});

Deno.test({
  name: "[node/fs.link] ASYNC: hard linking files passes error to callback",
  async fn() {
    let failed = false;
    await new Promise<void>((res, rej) => {
      link("no-such-file", "no-such-file", (err) => {
        if (err) rej(err);
        else res();
      });
    })
      .then(() => {
        fail("Expected to succeed");
      }, (err) => {
        assert(err);
        failed = true;
      });
    assert(failed);
  },
});

Deno.test({
  name: "[node/fs.linkSync] SYNC: hard linking files works as expected",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const linkedFile: string = tempFile + ".link";
    linkSync(tempFile, linkedFile);

    assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));
    Deno.removeSync(tempFile);
    Deno.removeSync(linkedFile);
  },
});

Deno.test("[node/fs.link] link callback isn't called twice if error is thrown", async () => {
  const tempDir = await Deno.makeTempDir();
  const tempFile = path.join(tempDir, "file.txt");
  const linkFile = path.join(tempDir, "link.txt");
  await Deno.writeTextFile(tempFile, "hello world");
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { link } from ${JSON.stringify(importUrl)}`,
    invocation: `link(${JSON.stringify(tempFile)},
                      ${JSON.stringify(linkFile)}, `,
    async cleanup() {
      await Deno.remove(tempDir, { recursive: true });
    },
  });
});

Deno.test("[node/fs.link] link accepts Buffer", async () => {
  const tempDir = await Deno.makeTempDir();
  const tempFile = path.join(tempDir, "file.txt");
  const linkedFile = path.join(tempDir, "file.link");
  const tempFileBuffer = Buffer.from(tempFile, "utf8");
  const linkedFileBuffer = Buffer.from(linkedFile, "utf8");
  await Deno.writeTextFile(tempFile, "hello world");

  await new Promise<void>((resolve, reject) => {
    link(tempFileBuffer, linkedFileBuffer, (err) => {
      if (err) reject(err);
      else resolve();
    });
  })
    .then(() => {
      assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));
    }, () => {
      fail("Expected to succeed");
    })
    .finally(() => {
      Deno.removeSync(tempFile);
      Deno.removeSync(linkedFile);
      Deno.removeSync(tempDir);
    });
});

Deno.test("[node/fs.linkSync] linkSync accepts Buffer", () => {
  const tempDir = Deno.makeTempDirSync();
  const tempFile = path.join(tempDir, "file.txt");
  const linkedFile = path.join(tempDir, "file.link");
  const tempFileBuffer = Buffer.from(tempFile, "utf8");
  const linkedFileBuffer = Buffer.from(linkedFile, "utf8");
  Deno.writeTextFileSync(tempFile, "hello world");

  linkSync(tempFileBuffer, linkedFileBuffer);
  assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));

  Deno.removeSync(linkedFile);
  Deno.removeSync(tempFile);
  Deno.removeSync(tempDir);
});

// ==========
// unlink tests (from _fs_unlink_test.ts)
// ==========

Deno.test({
  name: "[node/fs.unlink] ASYNC: deleting a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<void>((resolve, reject) => {
      unlink(file, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(file), false), () => fail())
      .finally(() => {
        if (existsSync(file)) Deno.removeSync(file);
      });
  },
});

Deno.test({
  name: "[node/fs.unlinkSync] SYNC: Test deleting a file",
  fn() {
    const file = Deno.makeTempFileSync();
    unlinkSync(file);
    assertEquals(existsSync(file), false);
  },
});

Deno.test("[node/fs.unlink] unlink callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { unlink } from ${JSON.stringify(importUrl)}`,
    invocation: `unlink(${JSON.stringify(tempFile)}, `,
  });
});

Deno.test("[node/fs.unlink] unlink accepts Buffer path", async () => {
  const file = Deno.makeTempFileSync();
  const bufferPath = Buffer.from(file, "utf-8");
  await new Promise<void>((resolve, reject) => {
    unlink(bufferPath, (err) => {
      if (err) reject(err);
      resolve();
    });
  })
    .then(() => assertEquals(existsSync(file), false), () => fail());
});

Deno.test("[node/fs.unlinkSync] unlinkSync accepts Buffer path", () => {
  const file = Deno.makeTempFileSync();
  const bufferPath = Buffer.from(file, "utf-8");
  unlinkSync(bufferPath);
  assertEquals(existsSync(file), false);
});

Deno.test("[node/fs.unlink] unlink: convert Deno error to Node.js error", async () => {
  const dir = Deno.makeTempDirSync();
  const unlinkPath = join(dir, "non_existent_file");

  await new Promise<void>((resolve, reject) => {
    unlink(unlinkPath, (err) => {
      if (err) reject(err);
      resolve();
    });
  })
    .then(() => fail(), (err) => {
      assertEquals(err.code, "ENOENT");
      assertEquals(err.syscall, "unlink");
      assertEquals(err.path, unlinkPath);
    });
});

Deno.test("[node/fs.unlinkSync] unlinkSync: convert Deno error to Node.js error", () => {
  const dir = Deno.makeTempDirSync();
  const unlinkPath = join(dir, "non_existent_file");

  try {
    unlinkSync(unlinkPath);
    fail();
  } catch (err) {
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).code, "ENOENT");
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).syscall, "unlink");
    // deno-lint-ignore no-explicit-any
    assertEquals((err as any).path, unlinkPath);
  }
});

// -- ftruncate --

Deno.test({
  name: "[node/fs ftruncate] no callback function results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Argument of type 'number' is not assignable to parameter of type 'NoParamCallback'
        ftruncate(123, 0);
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "[node/fs ftruncate] truncate entire file contents",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = Deno.makeTempFileSync();
    await Deno.writeTextFile(filePath, "hello world");
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncate(file.rid, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
          assertEquals(fileInfo.size, 0);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => {
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "[node/fs ftruncate] truncate file to a size of precisely len bytes",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = Deno.makeTempFileSync();
    await Deno.writeTextFile(filePath, "hello world");
    using file = await Deno.open(filePath, {
      read: true,
      write: true,
      create: true,
    });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncate(file.rid, 3, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
          assertEquals(fileInfo.size, 3);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => {
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "[node/fs ftruncateSync] truncate entire file contents",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    Deno.writeFileSync(filePath, new TextEncoder().encode("hello world"));
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncateSync(file.rid);
      const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
      assertEquals(fileInfo.size, 0);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

Deno.test({
  name:
    "[node/fs ftruncateSync] truncate file to a size of precisely len bytes",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    Deno.writeFileSync(filePath, new TextEncoder().encode("hello world"));
    using file = Deno.openSync(filePath, {
      read: true,
      write: true,
      create: true,
    });

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      ftruncateSync(file.rid, 3);
      const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
      assertEquals(fileInfo.size, 3);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

// -- futimes --

const _randomDate = new Date(Date.now() + 1000);

Deno.test({
  name: "[node/fs futimes] change file timestamps",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = Deno.makeTempFileSync();
    using file = await Deno.open(filePath, { create: true, write: true });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      futimes(file.rid, _randomDate, _randomDate, (err: Error | null) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(
        () => {
          const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);
          assertEquals(fileInfo.mtime, _randomDate);
          assertEquals(fileInfo.atime, _randomDate);
        },
        () => {
          fail("No error expected");
        },
      )
      .finally(() => {
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "[node/fs futimes] should throw error if atime is infinity",
  fn() {
    assertThrows(
      () => {
        futimes(123, Infinity, 0, (_err: Error | null) => {});
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});

Deno.test({
  name: "[node/fs futimes] should throw error if atime is NaN",
  fn() {
    assertThrows(
      () => {
        futimes(123, "some string", 0, (_err: Error | null) => {});
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});

Deno.test({
  name: "[node/fs futimesSync] change file timestamps",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath, { create: true, write: true });

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      futimesSync(file.rid, _randomDate, _randomDate);

      const fileInfo: Deno.FileInfo = Deno.lstatSync(filePath);

      assertEquals(fileInfo.mtime, _randomDate);
      assertEquals(fileInfo.atime, _randomDate);
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

Deno.test({
  name: "[node/fs futimesSync] should throw error if atime is NaN",
  fn() {
    assertThrows(
      () => {
        futimesSync(123, "some string", 0);
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});

Deno.test({
  name: "[node/fs futimesSync] should throw error if atime is Infinity",
  fn() {
    assertThrows(
      () => {
        futimesSync(123, Infinity, 0);
      },
      Error,
      "invalid atime, must not be infinity or NaN",
    );
  },
});

Deno.test({
  name:
    "[node/fs/promises FileHandle.readLines] close after readLines should not throw",
  async fn() {
    const tempFile = Deno.makeTempFileSync();
    Deno.writeTextFileSync(tempFile, "line one\nline two\nline three\n");
    try {
      const fd = await open(tempFile, "r");
      fd.readLines();
      await fd.close();
    } finally {
      Deno.removeSync(tempFile);
    }
  },
});

Deno.test({
  name:
    "[node/fs/promises FileHandle.readLines] should iterate lines correctly",
  async fn() {
    const tempFile = Deno.makeTempFileSync();
    Deno.writeTextFileSync(tempFile, "line one\nline two\nline three\n");
    try {
      const fd = await open(tempFile, "r");
      const lines: string[] = [];
      for await (const line of fd.readLines()) {
        lines.push(line);
      }
      assertEquals(lines, ["line one", "line two", "line three"]);
      await fd.close();
    } finally {
      Deno.removeSync(tempFile);
    }
  },
});

Deno.test(
  "[node/fs Dir] Dir is disposable via Symbol.dispose",
  { permissions: { read: true } },
  () => {
    using dir = opendirSync(".");
    void dir;
  },
);

Deno.test(
  "[node/fs Dir] Dir is async-disposable via Symbol.asyncDispose",
  { permissions: { read: true } },
  async () => {
    await using dir = await fsPromises.opendir(".");
    void dir;
  },
);
