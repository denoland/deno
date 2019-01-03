import { test, assertEqual } from "../testing/mod.ts";
import * as datetime from "mod.ts";

test(function parseDateTime() {
  assertEqual(
    datetime.parseDateTime("01-03-2019 16:34", "mm-dd-yyyy hh:mm"),
    new Date(2019, 1, 3, 16, 34)
  );
});
test(function parseDate() {
  assertEqual(
    datetime.parseDateTime("01-03-2019", "mm-dd-yyyy"),
    new Date(2019, 1, 3)
  );
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
