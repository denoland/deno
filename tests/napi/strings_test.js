// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const strings = loadTestLibrary();

Deno.test("napi string utf8", function () {
  assertEquals(strings.test_utf8(""), "");
  assertEquals(strings.test_utf8("🦕"), "🦕");
});

Deno.test("napi string", function () {
  assertEquals(strings.test_utf16(""), "");
  assertEquals(strings.test_utf16("🦕"), "🦕");
});

Deno.test("napi string utf8 roundtrip", function () {
  assertEquals(strings.test_utf8_roundtrip(""), "");
  assertEquals(strings.test_utf8_roundtrip("hello"), "hello");
  assertEquals(strings.test_utf8_roundtrip("🦕"), "🦕");
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
