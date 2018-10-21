// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

function testFirstArgument(arg1, expectedSize) {
  const file = new File(arg1, "name");
  assert(file instanceof File);
  assertEqual(file.name, "name");
  assertEqual(file.size, expectedSize);
  assertEqual(file.type, "");
}

test(function fileEmptyFileBits() {
  testFirstArgument([], 0);
});

test(function fileStringFileBits() {
  testFirstArgument(["bits"], 4);
});

test(function fileUnicodeStringFileBits() {
  testFirstArgument(["𝓽𝓮𝔁𝓽"], 16);
});

test(function fileStringObjectFileBits() {
  // tslint:disable-next-line no-construct
  testFirstArgument([new String("string object")], 13);
});

test(function fileEmptyBlobFileBits() {
  testFirstArgument([new Blob()], 0);
});

test(function fileBlobFileBits() {
  testFirstArgument([new Blob(["bits"])], 4);
});

test(function fileEmptyFileFileBits() {
  testFirstArgument([new File([], "world.txt")], 0);
});

test(function fileFileFileBits() {
  testFirstArgument([new File(["bits"], "world.txt")], 4);
});

test(function fileArrayBufferFileBits() {
  testFirstArgument([new ArrayBuffer(8)], 8);
});

test(function fileTypedArrayFileBits() {
  testFirstArgument([new Uint8Array([0x50, 0x41, 0x53, 0x53])], 4);
});

test(function fileVariousFileBits() {
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

test(function fileNumberInFileBits() {
  testFirstArgument([12], 2);
});

test(function fileArrayInFileBits() {
  testFirstArgument([[1, 2, 3]], 5);
});

test(function fileObjectInFileBits() {
  // "[object Object]"
  testFirstArgument([{}], 15);
});

function testSecondArgument(arg2, expectedFileName) {
  const file = new File(["bits"], arg2);
  assert(file instanceof File);
  assertEqual(file.name, expectedFileName);
}

test(function fileUsingFileName() {
  testSecondArgument("dummy", "dummy");
});

test(function fileUsingSpecialCharacterInFileName() {
  testSecondArgument("dummy/foo", "dummy:foo");
});

test(function fileUsingNullFileName() {
  testSecondArgument(null, "null");
});

test(function fileUsingNumberFileName() {
  testSecondArgument(1, "1");
});

test(function fileUsingEmptyStringFileName() {
  testSecondArgument("", "");
});
