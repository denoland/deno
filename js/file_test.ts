// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEquals } from "./test_util.ts";

function testFirstArgument(arg1, expectedSize): void {
  const file = new File(arg1, "name");
  assert(file instanceof File);
  assertEquals(file.name, "name");
  assertEquals(file.size, expectedSize);
  assertEquals(file.type, "");
}

test(function fileEmptyFileBits(): void {
  testFirstArgument([], 0);
});

test(function fileStringFileBits(): void {
  testFirstArgument(["bits"], 4);
});

test(function fileUnicodeStringFileBits(): void {
  testFirstArgument(["𝓽𝓮𝔁𝓽"], 16);
});

test(function fileStringObjectFileBits(): void {
  testFirstArgument([new String("string object")], 13);
});

test(function fileEmptyBlobFileBits(): void {
  testFirstArgument([new Blob()], 0);
});

test(function fileBlobFileBits(): void {
  testFirstArgument([new Blob(["bits"])], 4);
});

test(function fileEmptyFileFileBits(): void {
  testFirstArgument([new File([], "world.txt")], 0);
});

test(function fileFileFileBits(): void {
  testFirstArgument([new File(["bits"], "world.txt")], 4);
});

test(function fileArrayBufferFileBits(): void {
  testFirstArgument([new ArrayBuffer(8)], 8);
});

test(function fileTypedArrayFileBits(): void {
  testFirstArgument([new Uint8Array([0x50, 0x41, 0x53, 0x53])], 4);
});

test(function fileVariousFileBits(): void {
  testFirstArgument(
    [
      "bits",
      new Blob(["bits"]),
      new Blob(),
      new Uint8Array([0x50, 0x41]),
      new Uint16Array([0x5353]),
      new Uint32Array([0x53534150])
    ],
    16
  );
});

test(function fileNumberInFileBits(): void {
  testFirstArgument([12], 2);
});

test(function fileArrayInFileBits(): void {
  testFirstArgument([[1, 2, 3]], 5);
});

test(function fileObjectInFileBits(): void {
  // "[object Object]"
  testFirstArgument([{}], 15);
});

function testSecondArgument(arg2, expectedFileName): void {
  const file = new File(["bits"], arg2);
  assert(file instanceof File);
  assertEquals(file.name, expectedFileName);
}

test(function fileUsingFileName(): void {
  testSecondArgument("dummy", "dummy");
});

test(function fileUsingSpecialCharacterInFileName(): void {
  testSecondArgument("dummy/foo", "dummy:foo");
});

test(function fileUsingNullFileName(): void {
  testSecondArgument(null, "null");
});

test(function fileUsingNumberFileName(): void {
  testSecondArgument(1, "1");
});

test(function fileUsingEmptyStringFileName(): void {
  testSecondArgument("", "");
});
