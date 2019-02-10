// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert, test } from "../testing/mod.ts";
import { isFormFile } from "./formfile.ts";

test(function multipartIsFormFile() {
  assert.equal(
    isFormFile({
      filename: "foo",
      type: "application/json"
    }),
    true
  );
  assert.equal(
    isFormFile({
      filename: "foo"
    }),
    false
  );
});
