// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const strings = loadTestLibrary();

Deno.test("napi string utf8", function () {
  assertEquals(strings.test_utf8(""), "");
  assertEquals(strings.test_utf8(""), "");
});

Deno.test("napi string", function () {
  assertEquals(strings.test_utf16(""), "");
  assertEquals(strings.test_utf16(""), "");
});

Deno.test("napi string utf8 roundtrip", function () {
  assertEquals(strings.test_utf8_roundtrip(""), "");
  assertEquals(strings.test_utf8_roundtrip("hello"), "hello");
  assertEquals(strings.test_utf8_roundtrip(""), "");
});

Deno.test("napi property key latin1", function () {
  assertEquals(strings.test_property_key_latin1(), 42);
});

Deno.test("napi property key utf8", function () {
  assertEquals(strings.test_property_key_utf8(), 42);
});

Deno.test("napi property key utf16", function () {
  assertEquals(strings.test_property_key_utf16(), 42);
});

Deno.test("napi string latin1 roundtrip", function () {
  assertEquals(strings.test_latin1_roundtrip("hello"), "hello");
  assertEquals(strings.test_latin1_roundtrip(""), "");
});

Deno.test("napi string utf16 roundtrip", function () {
  assertEquals(strings.test_utf16_roundtrip("hello"), "hello");
  assertEquals(strings.test_utf16_roundtrip(""), "");
});

Deno.test("napi external string latin1", function () {
  // Returns true if zero-copy (not copied), false if copied
  const zeroCopy = strings.test_external_latin1();
  // Either outcome is valid -- zero-copy is preferred but copy is acceptable
  assertEquals(typeof zeroCopy, "boolean");
});

Deno.test("napi external string utf16", function () {
  // Returns true if zero-copy (not copied), false if copied
  const zeroCopy = strings.test_external_utf16();
  // Either outcome is valid -- zero-copy is preferred but copy is acceptable
  assertEquals(typeof zeroCopy, "boolean");
});

Deno.test("napi external string finalizers keep per-resource state", async () => {
  if (typeof globalThis.gc !== "function") {
    return;
  }

  strings.test_external_string_finalizer_reset();
  let values = strings.test_external_string_finalizer_collisions();

  assertEquals(values[0].length, 4096);
  assertEquals(values[1].length, 4096);
  assertEquals(values[2].length, 4096);
  assertEquals(values[3].length, 4096);
  // rusty_v8 cannot distinguish two live resources with the same address,
  // length, and encoding. The first remains external and the second is copied.
  assertEquals(values.slice(4), [true, false, true, false]);

  const empty = strings.test_empty_external_string_finalizers();
  assertEquals(empty[0], "");
  assertEquals(empty[1], "");
  assertEquals(empty.slice(2), [false, false]);
  assertEquals(strings.test_external_string_finalizer_status(), [
    4,
    false,
    false,
    0b111010,
  ]);

  values = null;
  for (let i = 0; i < 100; i++) {
    globalThis.gc();
    await new Promise((resolve) => setTimeout(resolve, 0));
    if (strings.test_external_string_finalizer_status()[0] === 6) {
      break;
    }
  }

  assertEquals(strings.test_external_string_finalizer_status(), [
    6,
    false,
    false,
    0b111111,
  ]);
});
