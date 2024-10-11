// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, assertThrows } from "@std/assert";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  closeSync,
  constants,
  createWriteStream,
  existsSync,
  lstatSync,
  mkdtempSync,
  openSync,
  promises,
  readFileSync,
  readSync,
  Stats,
  statSync,
  writeFileSync,
} from "node:fs";
import {
  constants as fsPromiseConstants,
  cp,
  FileHandle,
  open,
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
