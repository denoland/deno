// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";
import { concat } from "../../../test_util/std/bytes/mod.ts";
import { decode } from "../../../test_util/std/encoding/utf8.ts";

unitTest(function blobString(): void {
  const b1 = new Blob(["Hello World"]);
  const str = "Test";
  const b2 = new Blob([b1, str]);
  assertEquals(b2.size, b1.size + str.length);
});

unitTest(function blobBuffer(): void {
  const buffer = new ArrayBuffer(12);
  const u8 = new Uint8Array(buffer);
  const f1 = new Float32Array(buffer);
  const b1 = new Blob([buffer, u8]);
  assertEquals(b1.size, 2 * u8.length);
  const b2 = new Blob([b1, f1]);
  assertEquals(b2.size, 3 * u8.length);
});

unitTest(function blobSlice(): void {
  const blob = new Blob(["Deno", "Foo"]);
  const b1 = blob.slice(0, 3, "Text/HTML");
  assert(b1 instanceof Blob);
  assertEquals(b1.size, 3);
  assertEquals(b1.type, "text/html");
  const b2 = blob.slice(-1, 3);
  assertEquals(b2.size, 0);
  const b3 = blob.slice(100, 3);
  assertEquals(b3.size, 0);
  const b4 = blob.slice(0, 10);
  assertEquals(b4.size, blob.size);
});

unitTest(function blobInvalidType(): void {
  const blob = new Blob(["foo"], {
    type: "\u0521",
  });

  assertEquals(blob.type, "");
});

unitTest(function blobShouldNotThrowError(): void {
  let hasThrown = false;

  try {
    // deno-lint-ignore no-explicit-any
    const options1: any = {
      ending: "utf8",
      hasOwnProperty: "hasOwnProperty",
    };
    const options2 = Object.create(null);
    new Blob(["Hello World"], options1);
    new Blob(["Hello World"], options2);
  } catch {
    hasThrown = true;
  }

  assertEquals(hasThrown, false);
});

/* TODO https://github.com/denoland/deno/issues/7540
unitTest(function nativeEndLine(): void {
  const options = {
    ending: "native",
  } as const;
  const blob = new Blob(["Hello\nWorld"], options);

  assertEquals(blob.size, Deno.build.os === "windows" ? 12 : 11);
});
*/

unitTest(async function blobText(): Promise<void> {
  const blob = new Blob(["Hello World"]);
  assertEquals(await blob.text(), "Hello World");
});

unitTest(async function blobStream(): Promise<void> {
  const blob = new Blob(["Hello World"]);
  const stream = blob.stream();
  assert(stream instanceof ReadableStream);
  const reader = stream.getReader();
  let bytes = new Uint8Array();
  const read = async (): Promise<void> => {
    const { done, value } = await reader.read();
    if (!done && value) {
      bytes = concat(bytes, value);
      return read();
    }
  };
  await read();
  assertEquals(decode(bytes), "Hello World");
});

unitTest(async function blobArrayBuffer(): Promise<void> {
  const uint = new Uint8Array([102, 111, 111]);
  const blob = new Blob([uint]);
  assertEquals(await blob.arrayBuffer(), uint.buffer);
});

unitTest(function blobConstructorNameIsBlob(): void {
  const blob = new Blob();
  assertEquals(blob.constructor.name, "Blob");
});
