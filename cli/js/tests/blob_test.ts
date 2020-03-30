// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

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

unitTest(function blobShouldNotThrowError(): void {
  let hasThrown = false;

  try {
    const options1: object = {
      ending: "utf8",
      hasOwnProperty: "hasOwnProperty",
    };
    const options2: object = Object.create(null);
    new Blob(["Hello World"], options1);
    new Blob(["Hello World"], options2);
  } catch {
    hasThrown = true;
  }

  assertEquals(hasThrown, false);
});

unitTest(function nativeEndLine(): void {
  const options: object = {
    ending: "native",
  };
  const blob = new Blob(["Hello\nWorld"], options);

  assertEquals(blob.size, Deno.build.os === "win" ? 12 : 11);
});

// TODO(qti3e) Test the stored data in a Blob after implementing FileReader API.
