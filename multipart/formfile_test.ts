// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import { isFormFile } from "./formfile.ts";

test(function multipartIsFormFile() {
  assertEq(
    isFormFile({
      filename: "foo",
      type: "application/json"
    }),
    true
  );
  assertEq(
    isFormFile({
      filename: "foo"
    }),
    false
  );
});
