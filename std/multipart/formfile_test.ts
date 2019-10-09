// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { isFormFile } from "./formfile.ts";

test(function multipartIsFormFile(): void {
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

test(function isFormFileShouldNotThrow(): void {
  assertEquals(
    isFormFile({
      filename: "foo",
      type: "application/json",
      hasOwnProperty: "bar"
    }),
    true
  );
  assertEquals(isFormFile(Object.create(null)), false);
});
