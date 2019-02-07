// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual, assert } from "../testing/mod.ts";
import * as datetime from "mod.ts";

test(function parseDateTime() {
  assertEqual(
    datetime.parseDateTime("01-03-2019 16:34", "mm-dd-yyyy hh:mm"),
    new Date(2019, 1, 3, 16, 34)
  );
  assertEqual(
    datetime.parseDateTime("03-01-2019 16:34", "dd-mm-yyyy hh:mm"),
    new Date(2019, 1, 3, 16, 34)
  );
  assertEqual(
    datetime.parseDateTime("2019-01-03 16:34", "yyyy-mm-dd hh:mm"),
    new Date(2019, 1, 3, 16, 34)
  );
  assertEqual(
    datetime.parseDateTime("16:34 01-03-2019", "hh:mm mm-dd-yyyy"),
    new Date(2019, 1, 3, 16, 34)
  );
  assertEqual(
    datetime.parseDateTime("16:34 03-01-2019", "hh:mm dd-mm-yyyy"),
    new Date(2019, 1, 3, 16, 34)
  );
  assertEqual(
    datetime.parseDateTime("16:34 2019-01-03", "hh:mm yyyy-mm-dd"),
    new Date(2019, 1, 3, 16, 34)
  );
});

test(function invalidParseDateTimeFormatThrows() {
  try {
    (datetime as any).parseDateTime("2019-01-01 00:00", "x-y-z");
    assert(false, "no exception was thrown");
  } catch (e) {
    assertEqual(e.message, "Invalid datetime format!");
  }
});

test(function parseDate() {
  assertEqual(
    datetime.parseDate("01-03-2019", "mm-dd-yyyy"),
    new Date(2019, 1, 3)
  );
  assertEqual(
    datetime.parseDate("03-01-2019", "dd-mm-yyyy"),
    new Date(2019, 1, 3)
  );
  assertEqual(
    datetime.parseDate("2019-01-03", "yyyy-mm-dd"),
    new Date(2019, 1, 3)
  );
});

test(function invalidParseDateFormatThrows() {
  try {
    (datetime as any).parseDate("2019-01-01", "x-y-z");
    assert(false, "no exception was thrown");
  } catch (e) {
    assertEqual(e.message, "Invalid date format!");
  }
});

test(function currentDayOfYear() {
  assertEqual(
    datetime.currentDayOfYear(),
    Math.ceil(new Date().getTime() / 86400000) -
      Math.floor(
        new Date().setFullYear(new Date().getFullYear(), 0, 1) / 86400000
      )
  );
});
