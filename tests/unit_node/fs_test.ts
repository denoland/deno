// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, assertThrows } from "@std/assert/mod.ts";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  constants,
  createWriteStream,
  existsSync,
  mkdtempSync,
  promises,
  readFileSync,
  Stats,
  statSync,
  writeFileSync,
} from "node:fs";
import { constants as fsPromiseConstants, cp } from "node:fs/promises";
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
    }, Deno.errors.PermissionDenied);
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
