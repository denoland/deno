// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import {
  lookup,
  normalizeContentType,
  extension,
  charset,
  extensions,
  types,
} from "./mod.ts";

test(function testLookup(): void {
  assertEquals(lookup("json"), "application/json");
  assertEquals(lookup(".md"), "text/markdown");
  assertEquals(lookup("folder/file.js"), "application/javascript");
  assertEquals(lookup("folder/.htaccess"), undefined);
  assertEquals(lookup("file.ts"), "application/typescript");
});

test(function testNormalizeContentType(): void {
  assertEquals(normalizeContentType("text/html"), "text/html; charset=utf-8");
  assertEquals(
    normalizeContentType("text/html; charset=iso-8859-1"),
    "text/html; charset=iso-8859-1"
  );
});

test(function testExtension(): void {
  assertEquals(extension("application/octet-stream"), "bin");
  assertEquals(extension("application/javascript"), "js");
  assertEquals(extension("text/html"), "html");
});

test(function testCharset(): void {
  assertEquals(charset("text/markdown"), "UTF-8");
  assertEquals(charset("text/css"), "UTF-8");
});

test(function testExtensions(): void {
  assertEquals(extensions.get("application/javascript"), ["js", "mjs"]);
  assertEquals(extensions.get("foo"), undefined);
});

test(function testTypes(): void {
  assertEquals(types.get("js"), "application/javascript");
  assertEquals(types.get("foo"), undefined);
});
