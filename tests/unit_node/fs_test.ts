// Copyright 2018-2025 the Deno authors. MIT license.

/// <reference lib="deno.ns" />
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  fail,
} from "@std/assert";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import {
  closeSync,
  constants,
  copyFileSync,
  createWriteStream,
  existsSync,
  fchmod,
  fchmodSync,
  fchown,
  fchownSync,
  lstatSync,
  mkdtempSync,
  openSync,
  promises,
  promises as fsPromises,
  readFileSync,
  readSync,
  Stats,
  statSync,
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
import process from "node:process";
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
  "[node/fs writeFileSync] write file throws error when encoding is not implemented",
  () => {
    const data = "Hello";
    const filename = mkdtempSync(join(tmpdir(), "foo-")) + "/test.txt";

    assertThrows(
      () => writeFileSync(filename, data, { encoding: "utf16le" }),
      'The value "utf16le" is invalid for option "encoding"',
    );
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
