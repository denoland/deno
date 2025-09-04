// Copyright 2018-2025 the Deno authors. MIT license.
import {
  O_APPEND,
  O_CREAT,
  O_DIRECTORY,
  O_EXCL,
  O_RDONLY,
  O_RDWR,
  O_SYNC,
  O_TRUNC,
  O_WRONLY,
} from "node:constants";
import { assertEquals, assertRejects, assertThrows, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import {
  closeSync,
  existsSync,
  open,
  openSync,
  readSync,
  writeFileSync,
  writeSync,
} from "node:fs";
import { open as openPromise } from "node:fs/promises";
import { join, parse } from "node:path";

const tempDir = parse(Deno.makeTempFileSync()).dir;

Deno.test({
  name: "ASYNC: open file",
  async fn() {
    const file = Deno.makeTempFileSync();
    let fd1: number;
    await new Promise<number>((resolve, reject) => {
      open(file, (err, fd) => {
        if (err) reject(err);
        resolve(fd);
      });
    })
      .then((fd) => {
        fd1 = fd;
      }, () => fail())
      .finally(() => closeSync(fd1));
  },
});

Deno.test({
  name: "SYNC: open file",
  fn() {
    const file = Deno.makeTempFileSync();
    const fd = openSync(file, "r");
    closeSync(fd);
  },
});

Deno.test({
  name: "open with string flag 'a'",
  fn() {
    const file = join(tempDir, "some_random_file");
    const fd = openSync(file, "a");
    assertEquals(typeof fd, "number");
    assertEquals(existsSync(file), true);
    closeSync(fd);
  },
});

Deno.test({
  name: "open with string flag 'ax'",
  fn() {
    const file = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file, "ax");
      },
      Error,
      `EEXIST: file already exists, open '${file}'`,
    );
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with string flag 'a+'",
  fn() {
    const file = join(tempDir, "some_random_file2");
    const fd = openSync(file, "a+");
    assertEquals(typeof fd, "number");
    assertEquals(existsSync(file), true);
    closeSync(fd);
  },
});

Deno.test({
  name: "open with string flag 'ax+'",
  fn() {
    const file = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file, "ax+");
      },
      Error,
      `EEXIST: file already exists, open '${file}'`,
    );
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with string flag 'as'",
  fn() {
    const file = join(tempDir, "some_random_file10");
    const fd = openSync(file, "as");
    assertEquals(existsSync(file), true);
    assertEquals(typeof fd, "number");
    closeSync(fd);
  },
});

Deno.test({
  name: "open with string flag 'as+'",
  fn() {
    const file = join(tempDir, "some_random_file10");
    const fd = openSync(file, "as+");
    assertEquals(existsSync(file), true);
    assertEquals(typeof fd, "number");
    closeSync(fd);
  },
});

Deno.test({
  name: "open with string flag 'r'",
  fn() {
    const file = join(tempDir, "some_random_file3");
    assertThrows(() => {
      openSync(file, "r");
    }, Error);
  },
});

Deno.test({
  name: "open with string flag 'r+'",
  fn() {
    const file = join(tempDir, "some_random_file4");
    assertThrows(() => {
      openSync(file, "r+");
    }, Error);
  },
});

Deno.test({
  name: "open with string flag 'w'",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "w");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    closeSync(fd);

    const file2 = join(tempDir, "some_random_file5");
    const fd2 = openSync(file2, "w");
    assertEquals(typeof fd2, "number");
    assertEquals(existsSync(file2), true);
    closeSync(fd2);
  },
});

Deno.test({
  name: "open with string flag 'wx'",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "w");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    closeSync(fd);

    const file2 = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file2, "wx");
      },
      Error,
      `EEXIST: file already exists, open '${file2}'`,
    );
  },
});

Deno.test({
  name: "open with string flag 'w+'",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "w+");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    closeSync(fd);

    const file2 = join(tempDir, "some_random_file6");
    const fd2 = openSync(file2, "w+");
    assertEquals(typeof fd2, "number");
    assertEquals(existsSync(file2), true);
    closeSync(fd2);
  },
});

Deno.test({
  name: "open with string flag 'wx+'",
  fn() {
    const file = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file, "wx+");
      },
      Error,
      `EEXIST: file already exists, open '${file}'`,
    );
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with numeric flag `O_APPEND | O_CREAT | O_WRONLY` ('a')",
  fn() {
    const file = join(tempDir, "some_random_file");
    const fd = openSync(file, O_APPEND | O_CREAT | O_WRONLY);
    assertEquals(typeof fd, "number");
    assertEquals(existsSync(file), true);
    closeSync(fd);
  },
});

Deno.test({
  name:
    "open with numeric flag `O_APPEND | O_CREAT | O_WRONLY | O_EXCL` ('ax')",
  fn() {
    const file = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file, O_APPEND | O_CREAT | O_WRONLY | O_EXCL);
      },
      Error,
      `EEXIST: file already exists, open '${file}'`,
    );
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with numeric flag `O_APPEND | O_CREAT | O_RDWR` ('a+')",
  fn() {
    const file = join(tempDir, "some_random_file2");
    const fd = openSync(file, O_APPEND | O_CREAT | O_RDWR);
    assertEquals(typeof fd, "number");
    assertEquals(existsSync(file), true);
    closeSync(fd);
  },
});

Deno.test({
  name: "open with numeric flag `O_APPEND | O_CREAT | O_RDWR | O_EXCL` ('ax+')",
  fn() {
    const file = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file, O_APPEND | O_CREAT | O_RDWR | O_EXCL);
      },
      Error,
      `EEXIST: file already exists, open '${file}'`,
    );
    Deno.removeSync(file);
  },
});

Deno.test({
  name:
    "open with numeric flag `O_APPEND | O_CREAT | O_WRONLY | O_SYNC` ('as')",
  fn() {
    const file = join(tempDir, "some_random_file10");
    const fd = openSync(file, O_APPEND | O_CREAT | O_WRONLY | O_SYNC);
    assertEquals(existsSync(file), true);
    assertEquals(typeof fd, "number");
    closeSync(fd);
  },
});

Deno.test({
  name: "open with numeric flag `O_APPEND | O_CREAT | O_RDWR | O_SYNC` ('as+')",
  fn() {
    const file = join(tempDir, "some_random_file10");
    const fd = openSync(file, O_APPEND | O_CREAT | O_RDWR | O_SYNC);
    assertEquals(existsSync(file), true);
    assertEquals(typeof fd, "number");
    closeSync(fd);
  },
});

Deno.test({
  name: "open with numeric flag `O_RDONLY` ('r')",
  fn() {
    const file = join(tempDir, "some_random_file3");
    assertThrows(() => {
      openSync(file, O_RDONLY);
    }, Error);
  },
});

Deno.test({
  name: "open with numeric flag `O_RDWR` ('r+')",
  fn() {
    const file = join(tempDir, "some_random_file4");
    assertThrows(() => {
      openSync(file, O_RDWR);
    }, Error);
  },
});

Deno.test({
  name: "open with numeric flag `O_TRUNC | O_CREAT | O_WRONLY` ('w')",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, O_TRUNC | O_CREAT | O_WRONLY);
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    closeSync(fd);

    const file2 = join(tempDir, "some_random_file5");
    const fd2 = openSync(file2, O_TRUNC | O_CREAT | O_WRONLY);
    assertEquals(typeof fd2, "number");
    assertEquals(existsSync(file2), true);
    closeSync(fd2);
  },
});

Deno.test({
  name: "open with numeric flag `O_TRUNC | O_CREAT | O_WRONLY | O_EXCL` ('wx')",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "w");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    closeSync(fd);

    const file2 = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file2, O_TRUNC | O_CREAT | O_WRONLY | O_EXCL);
      },
      Error,
      `EEXIST: file already exists, open '${file2}'`,
    );
  },
});

Deno.test({
  name: "open with numeric flag `O_TRUNC | O_CREAT | O_RDWR` ('w+')",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, O_TRUNC | O_CREAT | O_RDWR);
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    closeSync(fd);

    const file2 = join(tempDir, "some_random_file6");
    const fd2 = openSync(file2, O_TRUNC | O_CREAT | O_RDWR);
    assertEquals(typeof fd2, "number");
    assertEquals(existsSync(file2), true);
    closeSync(fd2);
  },
});

Deno.test({
  name: "open with numeric flag `O_TRUNC | O_CREAT | O_RDWR | O_EXCL` ('wx+')",
  fn() {
    const file = Deno.makeTempFileSync();
    assertThrows(
      () => {
        openSync(file, O_TRUNC | O_CREAT | O_RDWR | O_EXCL);
      },
      Error,
      `EEXIST: file already exists, open '${file}'`,
    );
    Deno.removeSync(file);
  },
});

Deno.test("[std/node/fs] open callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { open } from ${JSON.stringify(importUrl)}`,
    invocation: `open(${JSON.stringify(tempFile)}, `,
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });

  Deno.test({
    name: "SYNC: open file with flag set to 0 (readonly)",
    fn() {
      const file = Deno.makeTempFileSync();
      const fd = openSync(file, 0);
      closeSync(fd);
    },
  });
});

Deno.test("[std/node/fs] openSync with custom flag", {
  ignore: Deno.build.os === "windows",
}, async () => {
  const file = await Deno.makeTempFile();
  assertThrows(
    () => {
      // Should throw error if path is not a directory
      openSync(file, O_DIRECTORY as number);
    },
    Error,
    `ENOTDIR: not a directory, open '${file}'`,
  );
  await Deno.remove(file);
});

Deno.test("[std/node/fs] open with custom flag", {
  ignore: Deno.build.os === "windows",
}, async () => {
  const file = await Deno.makeTempFile();
  await assertRejects(
    async () => {
      // Should throw error if path is not a directory
      // `openPromise` uses `open` under the hood.
      await openPromise(file, O_DIRECTORY as number);
    },
    Error,
    `ENOTDIR: not a directory, open '${file}'`,
  );
  await Deno.remove(file);
});

let invalidFlag: number | undefined;
// On linux it refers to the `O_TMPFILE` constant in libc.
// It should throw EINVAL when it's not followed by `O_RDWR` or `O_WRONLY`.
// https://docs.rs/libc/latest/libc/constant.O_TMPFILE.html
if (Deno.build.os === "linux" || Deno.build.os === "android") {
  if (Deno.build.arch === "x86_64") {
    invalidFlag = 4_259_840;
  } else {
    invalidFlag = 4_210_688;
  }
} else if (Deno.build.os === "darwin") {
  // On macOS it's a random value that is not a valid flag.
  invalidFlag = 0x7FFFFFFF;
}

Deno.test("[std/node/fs] openSync throws on invalid flags", {
  ignore: Deno.build.os === "windows",
}, async () => {
  const file = await Deno.makeTempFile();
  assertThrows(
    () => {
      openSync(file, invalidFlag as number);
    },
    Error,
    `EINVAL: invalid argument, open '${file}'`,
  );
  await Deno.remove(file);
});

Deno.test("[std/node/fs] open throws on invalid flags", {
  ignore: Deno.build.os === "windows",
}, async () => {
  const file = await Deno.makeTempFile();
  await assertRejects(
    async () => {
      // `openPromise` uses `open` under the hood.
      await openPromise(file, invalidFlag as number);
    },
    Error,
    `EINVAL: invalid argument, open '${file}'`,
  );
  await Deno.remove(file);
});

Deno.test(
  "[std/node/fs] openSync: only enable read permission when a custom flag is not followed by file access flags",
  {
    ignore: Deno.build.os === "windows",
  },
  async () => {
    const path = await Deno.makeTempFile();
    writeFileSync(path, "Hello, world!");

    const fd = openSync(path, O_SYNC);
    readSync(fd, new Uint8Array(1), 0, 1, 0);
    assertThrows(
      () => {
        writeSync(fd, "This should fail");
      },
      Error,
    );

    closeSync(fd);
    await Deno.remove(path);
  },
);

Deno.test(
  "[std/node/fs] open: only enable read permission when a custom flag is not followed by file access flags",
  {
    ignore: Deno.build.os === "windows",
  },
  async () => {
    const path = await Deno.makeTempFile();
    writeFileSync(path, "Hello, world!");

    const fileHandle = await openPromise(path, O_SYNC);
    readSync(fileHandle.fd, new Uint8Array(1), 0, 1, 0);
    assertThrows(
      () => {
        writeSync(fileHandle.fd, "This should fail");
      },
      Error,
    );

    await fileHandle.close();
    await Deno.remove(path);
  },
);

Deno.test(
  "[std/node/fs] open: only enable write permission",
  async () => {
    const path = await Deno.makeTempFile();
    const fileHandle = await openPromise(path, O_WRONLY);

    writeSync(fileHandle.fd, "Hello, world!");
    assertThrows(
      () => {
        readSync(fileHandle.fd, new Uint8Array(1), 0, 1, 0);
      },
      Error,
    );

    await fileHandle.close();
    await Deno.remove(path);
  },
);

Deno.test(
  "[std/node/fs] openSync: only enable write permission",
  async () => {
    const path = await Deno.makeTempFile();
    const fd = openSync(path, O_WRONLY);

    writeSync(fd, "Hello, world!");
    assertThrows(
      () => {
        readSync(fd, new Uint8Array(1), 0, 1, 0);
      },
      Error,
    );

    closeSync(fd);
    await Deno.remove(path);
  },
);
