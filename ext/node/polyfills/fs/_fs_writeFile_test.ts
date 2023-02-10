// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertRejects,
} from "../../testing/asserts.ts";
import { writeFile } from "./promises.ts";
import type { TextEncodings } from "../_utils.ts";
import { isWindows } from "../../_util/os.ts";

const decoder = new TextDecoder("utf-8");

Deno.test("Invalid encoding results in error()", async function testEncodingErrors() {
  await assertRejects(
    async () => {
      await writeFile(
        "some/path",
        "some data",
        "made-up-encoding" as TextEncodings,
      );
    },
    Error,
    `The value "made-up-encoding" is invalid for option "encoding"`,
  );
  await assertRejects(
    async () => {
      await writeFile("some/path", "some data", {
        encoding: "made-up-encoding" as TextEncodings,
      });
    },
    Error,
    `The value "made-up-encoding" is invalid for option "encoding"`,
  );
});

Deno.test(
  "Unsupported encoding results in error()",
  async function testUnsupportedEncoding() {
    await assertRejects(
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
    const file: Deno.FsFile = await Deno.open(tempFile, {
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
  if (isWindows) return;
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
    if (isWindows) return;

    const filename: string = await Deno.makeTempFile();
    const file: Deno.FsFile = await Deno.open(filename, {
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
