// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertRejects,
  assertThrows,
} from "@std/assert";
import { writeFile, writeFileSync } from "node:fs";
import * as path from "@std/path";

type TextEncodings =
  | "ascii"
  | "utf8"
  | "utf-8"
  | "utf16le"
  | "ucs2"
  | "ucs-2"
  | "base64"
  | "latin1"
  | "hex";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testDataDir = path.resolve(moduleDir, "testdata");
const decoder = new TextDecoder("utf-8");

Deno.test("Callback must be a function error", function fn() {
  assertThrows(
    () => {
      // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
      writeFile("some/path", "some data", "utf8");
    },
    TypeError,
    "Callback must be a function.",
  );
});

Deno.test("Invalid encoding results in error()", function testEncodingErrors() {
  assertThrows(
    () => {
      // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
      writeFile("some/path", "some data", "made-up-encoding", () => {});
    },
    Error,
    `The value "made-up-encoding" is invalid for option "encoding"`,
  );

  assertThrows(
    () => {
      // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
      writeFileSync("some/path", "some data", "made-up-encoding");
    },
    Error,
    `The value "made-up-encoding" is invalid for option "encoding"`,
  );

  assertThrows(
    () => {
      writeFile(
        "some/path",
        "some data",
        {
          // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
          encoding: "made-up-encoding",
        },
        () => {},
      );
    },
    Error,
    `The value "made-up-encoding" is invalid for option "encoding"`,
  );

  assertThrows(
    () => {
      writeFileSync("some/path", "some data", {
        // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
        encoding: "made-up-encoding",
      });
    },
    Error,
    `The value "made-up-encoding" is invalid for option "encoding"`,
  );
});

Deno.test(
  "Unsupported encoding results in error()",
  function testUnsupportedEncoding() {
    assertThrows(
      () => {
        writeFile("some/path", "some data", "utf16le", () => {});
      },
      Error,
      `Not implemented: "utf16le" encoding`,
    );

    assertThrows(
      () => {
        writeFileSync("some/path", "some data", "utf16le");
      },
      Error,
      `Not implemented: "utf16le" encoding`,
    );
  },
);

Deno.test(
  {
    name: "Data is written to correct rid",
    // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
    // The fs APIs should be rewritten to use actual FDs, not RIDs
    ignore: true,
  },
  async function testCorrectWriteUsingRid() {
    const tempFile: string = await Deno.makeTempFile();
    using file = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      writeFile(file.rid, "hello world", (err) => {
        if (err) return reject(err);
        resolve();
      });
    });

    const data = await Deno.readFile(tempFile);
    await Deno.remove(tempFile);
    assertEquals(decoder.decode(data), "hello world");
  },
);

Deno.test(
  "Data is written to correct file",
  async function testCorrectWriteUsingPath() {
    const res = await new Promise((resolve) => {
      writeFile("_fs_writeFile_test_file.txt", "hello world", resolve);
    });

    const data = await Deno.readFile("_fs_writeFile_test_file.txt");
    await Deno.remove("_fs_writeFile_test_file.txt");
    assertEquals(res, null);
    assertEquals(decoder.decode(data), "hello world");
  },
);

Deno.test(
  "Data is written to correct file encodings",
  async function testCorrectWriteUsingDifferentEncodings() {
    const encodings = [
      ["hex", "68656c6c6f20776f726c64"],
      ["HEX", "68656c6c6f20776f726c64"],
      ["base64", "aGVsbG8gd29ybGQ="],
      ["BASE64", "aGVsbG8gd29ybGQ="],
      ["utf8", "hello world"],
      ["utf-8", "hello world"],
    ];

    for (const [encoding, value] of encodings) {
      const res = await new Promise((resolve) => {
        writeFile(
          "_fs_writeFile_test_file.txt",
          value,
          encoding as TextEncodings,
          resolve,
        );
      });

      const data = await Deno.readFile("_fs_writeFile_test_file.txt");
      await Deno.remove("_fs_writeFile_test_file.txt");
      assertEquals(res, null);
      assertEquals(decoder.decode(data), "hello world");
    }
  },
);

Deno.test("Path can be an URL", async function testCorrectWriteUsingURL() {
  const url = new URL(
    Deno.build.os === "windows"
      ? "file:///" +
        path
          .join(testDataDir, "_fs_writeFile_test_file_url.txt")
          .replace(/\\/g, "/")
      : "file://" + path.join(testDataDir, "_fs_writeFile_test_file_url.txt"),
  );
  const filePath = path.fromFileUrl(url);
  const res = await new Promise((resolve) => {
    writeFile(url, "hello world", resolve);
  });
  assert(res === null);

  const data = await Deno.readFile(filePath);
  await Deno.remove(filePath);
  assertEquals(res, null);
  assertEquals(decoder.decode(data), "hello world");
});

Deno.test({
  name: "Mode is correctly set",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
}, async function testCorrectFileMode() {
  if (Deno.build.os === "windows") return;
  const filename = "_fs_writeFile_test_file.txt";

  const res = await new Promise((resolve) => {
    writeFile(filename, "hello world", { mode: 0o777 }, resolve);
  });

  const fileInfo = await Deno.stat(filename);
  await Deno.remove(filename);
  assertEquals(res, null);
  assert(fileInfo && fileInfo.mode);
  assertEquals(fileInfo.mode & 0o777, 0o777);
});

Deno.test(
  {
    name: "Mode is not set when rid is passed",
    // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
    // The fs APIs should be rewritten to use actual FDs, not RIDs
    ignore: true,
  },
  async function testCorrectFileModeRid() {
    if (Deno.build.os === "windows") return;

    const filename: string = await Deno.makeTempFile();
    using file = await Deno.open(filename, {
      create: true,
      write: true,
      read: true,
    });

    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      writeFile(file.rid, "hello world", { mode: 0o777 }, (err) => {
        if (err) return reject(err);
        resolve();
      });
    });

    const fileInfo = await Deno.stat(filename);
    await Deno.remove(filename);
    assert(fileInfo.mode);
    assertNotEquals(fileInfo.mode & 0o777, 0o777);
  },
);

Deno.test(
  "Is cancellable with an AbortSignal",
  async function testIsCancellableWithAbortSignal() {
    const tempFile: string = await Deno.makeTempFile();
    const controller = new AbortController();
    // The "as any" is necessary due to https://github.com/denoland/deno/issues/19527
    // deno-lint-ignore no-explicit-any
    const signal = controller.signal as any;

    const writeFilePromise = new Promise<void>((resolve, reject) => {
      writeFile(tempFile, "hello world", { signal }, (err) => {
        if (err) return reject(err);
        resolve();
      });
    });
    controller.abort();

    await assertRejects(
      () => writeFilePromise,
      "AbortError",
    );

    Deno.removeSync(tempFile);
  },
);

Deno.test(
  {
    name: "Data is written synchronously to correct rid",
    // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
    // The fs APIs should be rewritten to use actual FDs, not RIDs
    ignore: true,
  },
  function testCorrectWriteSyncUsingRid() {
    const tempFile: string = Deno.makeTempFileSync();
    using file = Deno.openSync(tempFile, {
      create: true,
      write: true,
      read: true,
    });

    // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
    writeFileSync(file.rid, "hello world");

    const data = Deno.readFileSync(tempFile);
    Deno.removeSync(tempFile);
    assertEquals(decoder.decode(data), "hello world");
  },
);

Deno.test(
  "Data is written to correct file encodings",
  function testCorrectWriteSyncUsingDifferentEncodings() {
    const encodings = [
      ["hex", "68656c6c6f20776f726c64"],
      ["HEX", "68656c6c6f20776f726c64"],
      ["base64", "aGVsbG8gd29ybGQ="],
      ["BASE64", "aGVsbG8gd29ybGQ="],
      ["utf8", "hello world"],
      ["utf-8", "hello world"],
    ];

    for (const [encoding, value] of encodings) {
      const file = "_fs_writeFileSync_test_file";
      writeFileSync(file, value, encoding as TextEncodings);

      const data = Deno.readFileSync(file);
      Deno.removeSync(file);
      assertEquals(decoder.decode(data), "hello world");
    }
  },
);

Deno.test(
  "Data is written synchronously to correct file",
  function testCorrectWriteSyncUsingPath() {
    const file = "_fs_writeFileSync_test_file";

    writeFileSync(file, "hello world");

    const data = Deno.readFileSync(file);
    Deno.removeSync(file);
    assertEquals(decoder.decode(data), "hello world");
  },
);

Deno.test("sync: Path can be an URL", function testCorrectWriteSyncUsingURL() {
  const filePath = path.join(
    testDataDir,
    "_fs_writeFileSync_test_file_url.txt",
  );
  const url = new URL(
    Deno.build.os === "windows"
      ? "file:///" + filePath.replace(/\\/g, "/")
      : "file://" + filePath,
  );
  writeFileSync(url, "hello world");

  const data = Deno.readFileSync(filePath);
  Deno.removeSync(filePath);
  assertEquals(decoder.decode(data), "hello world");
});

Deno.test(
  "Mode is correctly set when writing synchronously",
  function testCorrectFileModeSync() {
    if (Deno.build.os === "windows") return;
    const filename = "_fs_writeFileSync_test_file.txt";

    writeFileSync(filename, "hello world", { mode: 0o777 });

    const fileInfo = Deno.statSync(filename);
    Deno.removeSync(filename);
    assert(fileInfo && fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
  },
);
