// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi instanceof", function () {
  class Foo {}
  const foo = new Foo();
  assertEquals(lib.test_instanceof(foo, Foo), true);
  assertEquals(lib.test_instanceof(foo, Object), true);
  assertEquals(lib.test_instanceof({}, Foo), false);
  assertEquals(lib.test_instanceof(42, Object), false);
});

Deno.test("napi get_version", function () {
  const version = lib.test_get_version();
  assert(version >= 1);
});

Deno.test("napi run_script", function () {
  const result = lib.test_run_script("1 + 2");
  assertEquals(result, 3);

  const str = lib.test_run_script("'hello' + ' ' + 'world'");
  assertEquals(str, "hello world");
});

Deno.test("napi get_node_version", function () {
  const major = lib.test_get_node_version();
  assert(major >= 1);
});

Deno.test("napi get_last_error_info", function () {
  assertEquals(lib.test_get_last_error_info(), true);
});

Deno.test("napi get_module_file_name", function () {
  const filename = lib.test_get_module_file_name();
  assert(typeof filename === "string");
  assert(filename.length > 0);
});
