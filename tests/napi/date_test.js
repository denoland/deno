// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const date = loadTestLibrary();

Deno.test("napi date", function () {
  const dateTypeTestDate = date.createDate(1549183351);
  assertEquals(date.isDate(dateTypeTestDate), true);
  assertEquals(date.isDate(new Date(1549183351)), true);
  assertEquals(date.isDate(2.4), false);
  assertEquals(date.isDate("not a date"), false);
  assertEquals(date.isDate(undefined), false);
  assertEquals(date.isDate(null), false);
  assertEquals(date.isDate({}), false);
  assertEquals(date.getDateValue(new Date(1549183351)), 1549183351);
});
