// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrowsAsync,
} from "../../../testing/asserts.ts";
import { writeFile } from "./_fs_writeFile.ts";
import type { TextEncodings } from "../../_utils.ts";

const decoder = new TextDecoder("utf-8");

Deno.test("Invalid encoding results in error()", function testEncodingErrors() {
  assertThrowsAsync(
    async () => {
      // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
      await writeFile("some/path", "some data", "made-up-encoding");
    },
    Error,
    `The value "made-up-encoding" is invalid for option "encoding"`,
  );
  assertThrowsAsync(
    async () => {
      await writeFile("some/path", "some data", {
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
    assertThrowsAsync(
      async () => {
        await writeFile("some/path", "some data", "utf16le");
      },
      Error,
      `Not implemented: "utf16le" encoding`,
    );
  },
);

Deno.test(
  "Data is written to correct rid",
  async function testCorrectWriteUsingRid() {
    const tempFile: string = await Deno.makeTempFile();
    const file: Deno.File = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });

    await writeFile(file.rid, "hello world");
    Deno.close(file.rid);

    const data = await Deno.readFile(tempFile);
    await Deno.remove(tempFile);
    assertEquals(decoder.decode(data), "hello world");
  },
);

Deno.test(
  "Data is written to correct file",
  async function testCorrectWriteUsingPath() {
    const openResourcesBeforeWrite: Deno.ResourceMap = Deno.resources();

    await writeFile("_fs_writeFile_test_file.txt", "hello world");

    assertEquals(Deno.resources(), openResourcesBeforeWrite);
    const data = await Deno.readFile("_fs_writeFile_test_file.txt");
    await Deno.remove("_fs_writeFile_test_file.txt");
    assertEquals(decoder.decode(data), "hello world");
  },
);

Deno.test(
  "Data is written to correct file encodings",
  async function testCorrectWritePromiseUsingDifferentEncodings() {
    const encodings = [
      ["hex", "68656c6c6f20776f726c64"],
      ["HEX", "68656c6c6f20776f726c64"],
      ["base64", "aGVsbG8gd29ybGQ="],
      ["BASE64", "aGVsbG8gd29ybGQ="],
      ["utf8", "hello world"],
      ["utf-8", "hello world"],
    ];

    for (const [encoding, value] of encodings) {
      await writeFile(
        "_fs_writeFile_test_file.txt",
        value,
        encoding as TextEncodings,
      );

      const data = await Deno.readFile("_fs_writeFile_test_file.txt");
      await Deno.remove("_fs_writeFile_test_file.txt");
      assertEquals(decoder.decode(data), "hello world");
    }
  },
);

Deno.test("Mode is correctly set", async function testCorrectFileMode() {
  if (Deno.build.os === "windows") return;
  const filename = "_fs_writeFile_test_file.txt";
  await writeFile(filename, "hello world", { mode: 0o777 });

  const fileInfo = await Deno.stat(filename);
  await Deno.remove(filename);
  assert(fileInfo && fileInfo.mode);
  assertEquals(fileInfo.mode & 0o777, 0o777);
});

Deno.test(
  "Mode is not set when rid is passed",
  async function testCorrectFileModeRid() {
    if (Deno.build.os === "windows") return;

    const filename: string = await Deno.makeTempFile();
    const file: Deno.File = await Deno.open(filename, {
      create: true,
      write: true,
      read: true,
    });

    await writeFile(file.rid, "hello world", { mode: 0o777 });
    Deno.close(file.rid);

    const fileInfo = await Deno.stat(filename);
    await Deno.remove(filename);
    assert(fileInfo.mode);
    assertNotEquals(fileInfo.mode & 0o777, 0o777);
  },
);
