// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, test } from "checkin:testing";
import { throwCustomErrorWithCode } from "checkin:error";

test(function additionalPropertyIsWritable() {
  try {
    throwCustomErrorWithCode("foo", 1);
  } catch (e) {
    assertEquals(e.message, "foo");
    assertEquals(e.code, 1);
    e.code = 2;
  }
});
