// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";

// deno-lint-ignore no-explicit-any
function testFirstArgument(arg1: any[], expectedSize: number): void {
  const file = new File(arg1, "name");
  assert(file instanceof File);
  assertEquals(file.name, "name");
  assertEquals(file.size, expectedSize);
  assertEquals(file.type, "");
}

unitTest(function fileEmptyFileBits(): void {
  testFirstArgument([], 0);
});

unitTest(function fileStringFileBits(): void {
  testFirstArgument(["bits"], 4);
});

unitTest(function fileUnicodeStringFileBits(): void {
  testFirstArgument(["𝓽𝓮𝔁𝓽"], 16);
});

unitTest(function fileStringObjectFileBits(): void {
  testFirstArgument([new String("string object")], 13);
});

unitTest(function fileEmptyBlobFileBits(): void {
  testFirstArgument([new Blob()], 0);
});

unitTest(function fileBlobFileBits(): void {
  testFirstArgument([new Blob(["bits"])], 4);
});

unitTest(function fileEmptyFileFileBits(): void {
  testFirstArgument([new File([], "world.txt")], 0);
});

unitTest(function fileFileFileBits(): void {
  testFirstArgument([new File(["bits"], "world.txt")], 4);
});

unitTest(function fileArrayBufferFileBits(): void {
  testFirstArgument([new ArrayBuffer(8)], 8);
});

unitTest(function fileTypedArrayFileBits(): void {
  testFirstArgument([new Uint8Array([0x50, 0x41, 0x53, 0x53])], 4);
});

unitTest(function fileVariousFileBits(): void {
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

unitTest(function fileNumberInFileBits(): void {
  testFirstArgument([12], 2);
});

unitTest(function fileArrayInFileBits(): void {
  testFirstArgument([[1, 2, 3]], 5);
});

unitTest(function fileObjectInFileBits(): void {
  // "[object Object]"
  testFirstArgument([{}], 15);
});

// deno-lint-ignore no-explicit-any
function testSecondArgument(arg2: any, expectedFileName: string): void {
  const file = new File(["bits"], arg2);
  assert(file instanceof File);
  assertEquals(file.name, expectedFileName);
}

unitTest(function fileUsingFileName(): void {
  testSecondArgument("dummy", "dummy");
});

unitTest(function fileUsingSpecialCharacterInFileName(): void {
  testSecondArgument("dummy/foo", "dummy:foo");
});

unitTest(function fileUsingNullFileName(): void {
  testSecondArgument(null, "null");
});

unitTest(function fileUsingNumberFileName(): void {
  testSecondArgument(1, "1");
});

unitTest(function fileUsingEmptyStringFileName(): void {
  testSecondArgument("", "");
});

unitTest(
  { perms: { read: true, write: true } },
  function fileTruncateSyncSuccess(): void {
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
  async function fileTruncateSuccess(): Promise<void> {
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

unitTest({ perms: { read: true } }, function fileStatSyncSuccess(): void {
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

unitTest({ perms: { read: true } }, async function fileStatSuccess(): Promise<
  void
> {
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
