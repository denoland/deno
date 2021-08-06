// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";

// deno-lint-ignore no-explicit-any
function testFirstArgument(arg1: any[], expectedSize: number) {
  const file = new File(arg1, "name");
  assert(file instanceof File);
  assertEquals(file.name, "name");
  assertEquals(file.size, expectedSize);
  assertEquals(file.type, "");
}

unitTest(function fileEmptyFileBits() {
  testFirstArgument([], 0);
});

unitTest(function fileStringFileBits() {
  testFirstArgument(["bits"], 4);
});

unitTest(function fileUnicodeStringFileBits() {
  testFirstArgument(["𝓽𝓮𝔁𝓽"], 16);
});

unitTest(function fileStringObjectFileBits() {
  testFirstArgument([new String("string object")], 13);
});

unitTest(function fileEmptyBlobFileBits() {
  testFirstArgument([new Blob()], 0);
});

unitTest(function fileBlobFileBits() {
  testFirstArgument([new Blob(["bits"])], 4);
});

unitTest(function fileEmptyFileFileBits() {
  testFirstArgument([new File([], "world.txt")], 0);
});

unitTest(function fileFileFileBits() {
  testFirstArgument([new File(["bits"], "world.txt")], 4);
});

unitTest(function fileArrayBufferFileBits() {
  testFirstArgument([new ArrayBuffer(8)], 8);
});

unitTest(function fileTypedArrayFileBits() {
  testFirstArgument([new Uint8Array([0x50, 0x41, 0x53, 0x53])], 4);
});

unitTest(function fileVariousFileBits() {
  testFirstArgument(
    [
      "bits",
      new Blob(["bits"]),
      new Blob(),
      new Uint8Array([0x50, 0x41]),
      new Uint16Array([0x5353]),
      new Uint32Array([0x53534150]),
    ],
    16,
  );
});

unitTest(function fileNumberInFileBits() {
  testFirstArgument([12], 2);
});

unitTest(function fileArrayInFileBits() {
  testFirstArgument([[1, 2, 3]], 5);
});

unitTest(function fileObjectInFileBits() {
  // "[object Object]"
  testFirstArgument([{}], 15);
});

// deno-lint-ignore no-explicit-any
function testSecondArgument(arg2: any, expectedFileName: string) {
  const file = new File(["bits"], arg2);
  assert(file instanceof File);
  assertEquals(file.name, expectedFileName);
}

unitTest(function fileUsingFileName() {
  testSecondArgument("dummy", "dummy");
});

unitTest(function fileUsingNullFileName() {
  testSecondArgument(null, "null");
});

unitTest(function fileUsingNumberFileName() {
  testSecondArgument(1, "1");
});

unitTest(function fileUsingEmptyStringFileName() {
  testSecondArgument("", "");
});

unitTest(
  { perms: { read: true, write: true } },
  function fileTruncateSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fileTruncateSync.txt";
    const file = Deno.openSync(filename, {
      create: true,
      read: true,
      write: true,
    });

    file.truncateSync(20);
    assertEquals(Deno.readFileSync(filename).byteLength, 20);
    file.truncateSync(5);
    assertEquals(Deno.readFileSync(filename).byteLength, 5);
    file.truncateSync(-5);
    assertEquals(Deno.readFileSync(filename).byteLength, 0);

    file.close();
    Deno.removeSync(filename);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function fileTruncateSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fileTruncate.txt";
    const file = await Deno.open(filename, {
      create: true,
      read: true,
      write: true,
    });

    await file.truncate(20);
    assertEquals((await Deno.readFile(filename)).byteLength, 20);
    await file.truncate(5);
    assertEquals((await Deno.readFile(filename)).byteLength, 5);
    await file.truncate(-5);
    assertEquals((await Deno.readFile(filename)).byteLength, 0);

    file.close();
    await Deno.remove(filename);
  },
);

unitTest({ perms: { read: true } }, function fileStatSyncSuccess() {
  const file = Deno.openSync("README.md");
  const fileInfo = file.statSync();
  assert(fileInfo.isFile);
  assert(!fileInfo.isSymlink);
  assert(!fileInfo.isDirectory);
  assert(fileInfo.size);
  assert(fileInfo.atime);
  assert(fileInfo.mtime);
  // The `birthtime` field is not available on Linux before kernel version 4.11.
  assert(fileInfo.birthtime || Deno.build.os === "linux");

  file.close();
});

unitTest({ perms: { read: true } }, async function fileStatSuccess() {
  const file = await Deno.open("README.md");
  const fileInfo = await file.stat();
  assert(fileInfo.isFile);
  assert(!fileInfo.isSymlink);
  assert(!fileInfo.isDirectory);
  assert(fileInfo.size);
  assert(fileInfo.atime);
  assert(fileInfo.mtime);
  // The `birthtime` field is not available on Linux before kernel version 4.11.
  assert(fileInfo.birthtime || Deno.build.os === "linux");

  file.close();
});
