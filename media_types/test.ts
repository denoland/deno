// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import {
  lookup,
  contentType,
  extension,
  charset,
  extensions,
  types
} from "./mod.ts";

test(function testLookup() {
  assertEq(lookup("json"), "application/json");
  assertEq(lookup(".md"), "text/markdown");
  assertEq(lookup("folder/file.js"), "application/javascript");
  assertEq(lookup("folder/.htaccess"), undefined);
});

test(function testContentType() {
  assertEq(contentType("markdown"), "text/markdown; charset=utf-8");
  assertEq(contentType("file.json"), "application/json; charset=utf-8");
  assertEq(contentType("text/html"), "text/html; charset=utf-8");
  assertEq(
    contentType("text/html; charset=iso-8859-1"),
    "text/html; charset=iso-8859-1"
  );
  assertEq(contentType(".htaccess"), undefined);
});

test(function testExtension() {
  assertEq(extension("application/octet-stream"), "bin");
  assertEq(extension("application/javascript"), "js");
  assertEq(extension("text/html"), "html");
});

test(function testCharset() {
  assertEq(charset("text/markdown"), "UTF-8");
  assertEq(charset("text/css"), "UTF-8");
});

test(function testExtensions() {
  assertEq(extensions.get("application/javascript"), ["js", "mjs"]);
  assertEq(extensions.get("foo"), undefined);
});

test(function testTypes() {
  assertEq(types.get("js"), "application/javascript");
  assertEq(types.get("foo"), undefined);
});
