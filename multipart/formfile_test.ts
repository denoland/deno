// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { isFormFile } from "./formfile.ts";

test(function multipartIsFormFile() {
  assertEquals(
    isFormFile({
      filename: "foo",
      type: "application/json"
    }),
    true
  );
  assertEquals(
    isFormFile({
      filename: "foo"
    }),
    false
  );
});
