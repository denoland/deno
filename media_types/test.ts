// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { assertEqual, test } from "../testing/mod.ts";
import {
  lookup,
  contentType,
  extension,
  charset,
  extensions,
  types
} from "./mod.ts";

test(function testLookup() {
  assertEqual(lookup("json"), "application/json");
  assertEqual(lookup(".md"), "text/markdown");
  assertEqual(lookup("folder/file.js"), "application/javascript");
  assertEqual(lookup("folder/.htaccess"), undefined);
});

test(function testContentType() {
  assertEqual(contentType("markdown"), "text/markdown; charset=utf-8");
  assertEqual(contentType("file.json"), "application/json; charset=utf-8");
  assertEqual(contentType("text/html"), "text/html; charset=utf-8");
  assertEqual(
    contentType("text/html; charset=iso-8859-1"),
    "text/html; charset=iso-8859-1"
  );
  assertEqual(contentType(".htaccess"), undefined);
});

test(function testExtension() {
  assertEqual(extension("application/octet-stream"), "bin");
  assertEqual(extension("application/javascript"), "js");
  assertEqual(extension("text/html"), "html");
});

test(function testCharset() {
  assertEqual(charset("text/markdown"), "UTF-8");
  assertEqual(charset("text/css"), "UTF-8");
});

test(function testExtensions() {
  assertEqual(extensions.get("application/javascript"), ["js", "mjs"]);
  assertEqual(extensions.get("foo"), undefined);
});

test(function testTypes() {
  assertEqual(types.get("js"), "application/javascript");
  assertEqual(types.get("foo"), undefined);
});
