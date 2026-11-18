// Copyright 2018-2026 the Deno authors. MIT license.
import * as path from "@std/path";
import { assertEquals } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import assert from "node:assert";
import { constants, copyFile, copyFileSync, cpSync, existsSync } from "node:fs";
import { copyFile as copyFilePromise, cp } from "node:fs/promises";

const destFile = "./destination.txt";

Deno.test({
  name: "[std/node/fs] copy file",
  fn: async () => {
    const sourceFile = Deno.makeTempFileSync();
    const err = await new Promise((resolve) => {
      copyFile(sourceFile, destFile, (err?: Error | null) => resolve(err));
    });
    assert(!err);
    assert(existsSync(destFile));
    Deno.removeSync(sourceFile);
    Deno.removeSync(destFile);
  },
});

Deno.test({
  name: "[std/node/fs] copy file sync",
  fn: () => {
    const sourceFile = Deno.makeTempFileSync();
    copyFileSync(sourceFile, destFile);
    assert(existsSync(destFile));
    Deno.removeSync(sourceFile);
    Deno.removeSync(destFile);
  },
});

Deno.test("[std/node/fs] copyFile reports Node ENOENT metadata", async () => {
  const dir = await Deno.makeTempDir();
  const src = path.join(dir, "missing.txt");
  const dest = path.join(dir, "target.txt");
  const expected = {
    message:
      `ENOENT: no such file or directory, copyfile '${src}' -> '${dest}'`,
    code: "ENOENT",
    errno: Deno.build.os === "windows" ? -4058 : -2,
    syscall: "copyfile",
    path: src,
    dest,
  };

  try {
    for (const mode of [0, constants.COPYFILE_EXCL]) {
      await assert.rejects(copyFilePromise(src, dest, mode), expected);
      await assert.rejects(
        new Promise<void>((resolve, reject) => {
          copyFile(src, dest, mode, (err) => err ? reject(err) : resolve());
        }),
        expected,
      );
      assert.throws(() => copyFileSync(src, dest, mode), expected);
    }
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

Deno.test("[std/node/fs] exclusive copyFile reports Node EEXIST metadata", async () => {
  const dir = await Deno.makeTempDir();
  const src = path.join(dir, "source.txt");
  const dest = path.join(dir, "target.txt");
  const mode = constants.COPYFILE_EXCL;
  const expected = {
    message: `EEXIST: file already exists, copyfile '${src}' -> '${dest}'`,
    code: "EEXIST",
    errno: Deno.build.os === "windows" ? -4075 : -17,
    syscall: "copyfile",
    path: src,
    dest,
  };

  try {
    await Deno.writeTextFile(src, "source");
    await Deno.writeTextFile(dest, "target");
    await assert.rejects(copyFilePromise(src, dest, mode), expected);
    await assert.rejects(
      new Promise<void>((resolve, reject) => {
        copyFile(src, dest, mode, (err) => err ? reject(err) : resolve());
      }),
      expected,
    );
    assert.throws(() => copyFileSync(src, dest, mode), expected);
    assertEquals(await Deno.readTextFile(dest), "target");
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

Deno.test("[std/node/fs] copyFile callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertRejects won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  const tempFile1 = path.join(tempDir, "file1.txt");
  const tempFile2 = path.join(tempDir, "file2.txt");
  await Deno.writeTextFile(tempFile1, "hello world");
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { copyFile } from ${JSON.stringify(importUrl)}`,
    invocation: `copyFile(${JSON.stringify(tempFile1)},
                          ${JSON.stringify(tempFile2)}, `,
    async cleanup() {
      await Deno.remove(tempDir, { recursive: true });
    },
  });
});

Deno.test("[std/node/fs] cp creates destination directory", async () => {
  const tempDir = await Deno.makeTempDir();
  const tempFile1 = path.join(tempDir, "file1.txt");
  const tempFile2 = path.join(tempDir, "dir", "file2.txt");
  await Deno.writeTextFile(tempFile1, "hello world");
  cpSync(tempFile1, tempFile2);
  assert(existsSync(tempFile2));
  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("[std/node/fs] cpSync preserveTimestamps copies atime/mtime", async () => {
  const tempDir = await Deno.makeTempDir();
  const src = path.join(tempDir, "src.txt");
  const dest = path.join(tempDir, "dest.txt");
  const atime = new Date("2021-01-02T03:04:05.000Z");
  const mtime = new Date("2021-01-03T04:05:06.000Z");

  try {
    await Deno.writeTextFile(src, "hello");
    Deno.utimeSync(src, atime, mtime);

    cpSync(src, dest, { preserveTimestamps: true });

    const srcStat = Deno.statSync(src);
    const destStat = Deno.statSync(dest);
    assert(srcStat.atime);
    assert(srcStat.mtime);
    assert(destStat.atime);
    assert(destStat.mtime);

    assertEquals(destStat.atime.getTime(), srcStat.atime.getTime());
    assertEquals(destStat.mtime.getTime(), srcStat.mtime.getTime());
  } finally {
    await Deno.remove(tempDir, { recursive: true });
  }
});

Deno.test("[std/node/fs] cp preserveTimestamps copies atime/mtime", async () => {
  const tempDir = await Deno.makeTempDir();
  const src = path.join(tempDir, "src.txt");
  const dest = path.join(tempDir, "dest.txt");
  const atime = new Date("2021-01-02T03:04:05.000Z");
  const mtime = new Date("2021-01-03T04:05:06.000Z");

  try {
    await Deno.writeTextFile(src, "hello");
    Deno.utimeSync(src, atime, mtime);

    await cp(src, dest, { preserveTimestamps: true });

    const srcStat = Deno.statSync(src);
    const destStat = Deno.statSync(dest);
    assert(srcStat.atime);
    assert(srcStat.mtime);
    assert(destStat.atime);
    assert(destStat.mtime);

    assertEquals(destStat.atime.getTime(), srcStat.atime.getTime());
    assertEquals(destStat.mtime.getTime(), srcStat.mtime.getTime());
  } finally {
    await Deno.remove(tempDir, { recursive: true });
  }
});

Deno.test({
  name: "[std/node/fs] cpSync throws for socket source",
  ignore: Deno.build.os === "windows",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const src = path.join(tempDir, "source.sock");
    const dest = path.join(tempDir, "dest.sock");
    const listener = Deno.listen({ transport: "unix", path: src });

    try {
      assert.throws(() => cpSync(src, dest), {
        code: "ERR_FS_CP_SOCKET",
      });
    } finally {
      listener.close();
      await Deno.remove(tempDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "[std/node/fs] cp throws for socket source",
  ignore: Deno.build.os === "windows",
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const src = path.join(tempDir, "source.sock");
    const dest = path.join(tempDir, "dest.sock");
    const listener = Deno.listen({ transport: "unix", path: src });

    try {
      await assert.rejects(
        () => cp(src, dest),
        { code: "ERR_FS_CP_SOCKET" },
      );
    } finally {
      listener.close();
      await Deno.remove(tempDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "[std/node/fs] cpSync throws for FIFO source",
  ignore: Deno.build.os === "windows",
  permissions: { read: true, write: true, run: true },
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const src = path.join(tempDir, "source.fifo");
    const dest = path.join(tempDir, "dest.fifo");

    try {
      const result = new Deno.Command("mkfifo", { args: [src] }).outputSync();
      if (result.code !== 0) {
        throw new Error(
          `mkfifo failed: ${new TextDecoder().decode(result.stderr)}`,
        );
      }

      assert.throws(() => cpSync(src, dest), {
        code: "ERR_FS_CP_FIFO_PIPE",
      });
    } finally {
      await Deno.remove(tempDir, { recursive: true });
    }
  },
});

Deno.test({
  name: "[std/node/fs] cp throws for FIFO source",
  ignore: Deno.build.os === "windows",
  permissions: { read: true, write: true, run: true },
  fn: async () => {
    const tempDir = await Deno.makeTempDir();
    const src = path.join(tempDir, "source.fifo");
    const dest = path.join(tempDir, "dest.fifo");

    try {
      const result = new Deno.Command("mkfifo", { args: [src] }).outputSync();
      if (result.code !== 0) {
        throw new Error(
          `mkfifo failed: ${new TextDecoder().decode(result.stderr)}`,
        );
      }

      await assert.rejects(
        () => cp(src, dest),
        { code: "ERR_FS_CP_FIFO_PIPE" },
      );
    } finally {
      await Deno.remove(tempDir, { recursive: true });
    }
  },
});
