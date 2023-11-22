// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  O_APPEND,
  O_CREAT,
  O_EXCL,
  O_RDONLY,
  O_RDWR,
  O_SYNC,
  O_TRUNC,
  O_WRONLY,
} from "node:constants";
import {
  assert,
  assertEquals,
  assertThrows,
  fail,
} from "../../../../test_util/std/assert/mod.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { open, openSync } from "node:fs";
import { join, parse } from "node:path";
import { closeSync, existsSync } from "node:fs";

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
        assert(Deno.resources()[fd], `${fd}`);
      }, () => fail())
      .finally(() => closeSync(fd1));
  },
});

Deno.test({
  name: "SYNC: open file",
  fn() {
    const file = Deno.makeTempFileSync();
    const fd = openSync(file, "r");
    assert(Deno.resources()[fd]);
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
    assert(Deno.resources()[fd]);
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
    assert(Deno.resources()[fd]);
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
      assert(Deno.resources()[fd]);
      closeSync(fd);
    },
  });
});
