// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "./test_util.ts";

// deno-lint-ignore no-explicit-any
function testFirstArgument(arg1: any[], expectedSize: number): void {
  const file = new File(arg1, "name");
  assert(file instanceof File);
  assertEquals(file.name, "name");
  assertEquals(file.size, expectedSize);
  assertEquals(file.type, "");
}

Deno.test("fileEmptyFileBits", function (): void {
  testFirstArgument([], 0);
});

Deno.test("fileStringFileBits", function (): void {
  testFirstArgument(["bits"], 4);
});

Deno.test("fileUnicodeStringFileBits", function (): void {
  testFirstArgument(["𝓽𝓮𝔁𝓽"], 16);
});

Deno.test("fileStringObjectFileBits", function (): void {
  testFirstArgument([new String("string object")], 13);
});

Deno.test("fileEmptyBlobFileBits", function (): void {
  testFirstArgument([new Blob()], 0);
});

Deno.test("fileBlobFileBits", function (): void {
  testFirstArgument([new Blob(["bits"])], 4);
});

Deno.test("fileEmptyFileFileBits", function (): void {
  testFirstArgument([new File([], "world.txt")], 0);
});

Deno.test("fileFileFileBits", function (): void {
  testFirstArgument([new File(["bits"], "world.txt")], 4);
});

Deno.test("fileArrayBufferFileBits", function (): void {
  testFirstArgument([new ArrayBuffer(8)], 8);
});

Deno.test("fileTypedArrayFileBits", function (): void {
  testFirstArgument([new Uint8Array([0x50, 0x41, 0x53, 0x53])], 4);
});

Deno.test("fileVariousFileBits", function (): void {
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

Deno.test("fileNumberInFileBits", function (): void {
  testFirstArgument([12], 2);
});

Deno.test("fileArrayInFileBits", function (): void {
  testFirstArgument([[1, 2, 3]], 5);
});

Deno.test("fileObjectInFileBits", function (): void {
  // "[object Object]"
  testFirstArgument([{}], 15);
});

// deno-lint-ignore no-explicit-any
function testSecondArgument(arg2: any, expectedFileName: string): void {
  const file = new File(["bits"], arg2);
  assert(file instanceof File);
  assertEquals(file.name, expectedFileName);
}

Deno.test("fileUsingFileName", function (): void {
  testSecondArgument("dummy", "dummy");
});

Deno.test("fileUsingSpecialCharacterInFileName", function (): void {
  testSecondArgument("dummy/foo", "dummy:foo");
});

Deno.test("fileUsingNullFileName", function (): void {
  testSecondArgument(null, "null");
});

Deno.test("fileUsingNumberFileName", function (): void {
  testSecondArgument(1, "1");
});

Deno.test("fileUsingEmptyStringFileName", function (): void {
  testSecondArgument("", "");
});
