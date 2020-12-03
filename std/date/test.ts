// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import * as date from "./mod.ts";

Deno.test({
  name: "[std/date] parse",
  fn: () => {
    assertEquals(
      date.parse("01-03-2019 16:30", "MM-dd-yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      date.parse("01.03.2019 16:30", "MM.dd.yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      date.parse("01.03.2019 16:30", "MM.dd.yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      date.parse("03-01-2019 16:31", "dd-MM-yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 31),
    );
    assertEquals(
      date.parse("2019-01-03 16:32", "yyyy-MM-dd HH:mm"),
      new Date(2019, 0, 3, 16, 32),
    );
    assertEquals(
      date.parse("16:33 01-03-2019", "HH:mm MM-dd-yyyy"),
      new Date(2019, 0, 3, 16, 33),
    );
    assertEquals(
      date.parse("01-03-2019 16:33:23.123", "MM-dd-yyyy HH:mm:ss.SSS"),
      new Date(2019, 0, 3, 16, 33, 23, 123),
    );
    assertEquals(
      date.parse("01-03-2019 09:33 PM", "MM-dd-yyyy HH:mm a"),
      new Date(2019, 0, 3, 21, 33),
    );
    assertEquals(
      date.parse("16:34 03-01-2019", "HH:mm dd-MM-yyyy"),
      new Date(2019, 0, 3, 16, 34),
    );
    assertEquals(
      date.parse("16:35 2019-01-03", "HH:mm yyyy-MM-dd"),
      new Date(2019, 0, 3, 16, 35),
    );
    assertEquals(
      date.parse("01-03-2019", "MM-dd-yyyy"),
      new Date(2019, 0, 3),
    );
    assertEquals(
      date.parse("03-01-2019", "dd-MM-yyyy"),
      new Date(2019, 0, 3),
    );
    assertEquals(
      date.parse("2019-01-03", "yyyy-MM-dd"),
      new Date(2019, 0, 3),
    );
  },
});

Deno.test({
  name: "[std/date] invalidParseDateFormatThrows",
  fn: () => {
    assertThrows((): void => {
      // deno-lint-ignore no-explicit-any
      (date as any).parse("2019-01-01 00:00", "x-y-z");
    }, Error);
    assertThrows((): void => {
      // deno-lint-ignore no-explicit-any
      (date as any).parse("2019-01-01", "x-y-z");
    }, Error);
  },
});

Deno.test({
  name: "[std/date] format",
  fn: () => {
    // 00 hours
    assertEquals(
      "01:00:00",
      date.format(new Date("2019-01-01T01:00:00"), "HH:mm:ss"),
    );
    assertEquals(
      "13:00:00",
      date.format(new Date("2019-01-01T13:00:00"), "HH:mm:ss"),
    );

    // 12 hours
    assertEquals(
      "01:00:00",
      date.format(new Date("2019-01-01T01:00:00"), "hh:mm:ss"),
    );
    assertEquals(
      "01:00:00",
      date.format(new Date("2019-01-01T13:00:00"), "hh:mm:ss"),
    );

    // milliseconds
    assertEquals(
      "13:00:00.000",
      date.format(new Date("2019-01-01T13:00:00"), "HH:mm:ss.SSS"),
    );
    assertEquals(
      "13:00:00.000",
      date.format(new Date("2019-01-01T13:00:00.000"), "HH:mm:ss.SSS"),
    );
    assertEquals(
      "13:00:00.123",
      date.format(new Date("2019-01-01T13:00:00.123"), "HH:mm:ss.SSS"),
    );

    // day period
    assertEquals(
      "01:00:00 AM",
      date.format(new Date("2019-01-01T01:00:00"), "HH:mm:ss a"),
    );
    assertEquals(
      "01:00:00 AM",
      date.format(new Date("2019-01-01T01:00:00"), "hh:mm:ss a"),
    );
    assertEquals(
      "01:00:00 PM",
      date.format(new Date("2019-01-01T13:00:00"), "hh:mm:ss a"),
    );
    assertEquals(
      "21:00:00 PM",
      date.format(new Date("2019-01-01T21:00:00"), "HH:mm:ss a"),
    );
    assertEquals(
      "09:00:00 PM",
      date.format(new Date("2019-01-01T21:00:00"), "hh:mm:ss a"),
    );

    // quoted literal
    assertEquals(
      date.format(new Date(2019, 0, 20), "'today:' yyyy-MM-dd"),
      "today: 2019-01-20",
    );
  },
});

Deno.test({
  name: "[std/date] dayOfYear",
  fn: () => {
    // from https://golang.org/src/time/time_test.go
    // Test YearDay in several different scenarios
    // and corner cases
    // Non-leap-year tests
    assertEquals(date.dayOfYear(new Date("2007-01-01T00:00:00.000Z")), 1);
    assertEquals(date.dayOfYear(new Date("2007-01-15T00:00:00.000Z")), 15);
    assertEquals(date.dayOfYear(new Date("2007-02-01T00:00:00.000Z")), 32);
    assertEquals(date.dayOfYear(new Date("2007-02-15T00:00:00.000Z")), 46);
    assertEquals(date.dayOfYear(new Date("2007-03-01T00:00:00.000Z")), 60);
    assertEquals(date.dayOfYear(new Date("2007-03-15T00:00:00.000Z")), 74);
    assertEquals(date.dayOfYear(new Date("2007-04-01T00:00:00.000Z")), 91);
    assertEquals(date.dayOfYear(new Date("2007-12-31T00:00:00.000Z")), 365);

    // Leap-year tests
    assertEquals(date.dayOfYear(new Date("2008-01-01T00:00:00.000Z")), 1);
    assertEquals(date.dayOfYear(new Date("2008-01-15T00:00:00.000Z")), 15);
    assertEquals(date.dayOfYear(new Date("2008-02-01T00:00:00.000Z")), 32);
    assertEquals(date.dayOfYear(new Date("2008-02-15T00:00:00.000Z")), 46);
    assertEquals(date.dayOfYear(new Date("2008-03-01T00:00:00.000Z")), 61);
    assertEquals(date.dayOfYear(new Date("2008-03-15T00:00:00.000Z")), 75);
    assertEquals(date.dayOfYear(new Date("2008-04-01T00:00:00.000Z")), 92);
    assertEquals(date.dayOfYear(new Date("2008-12-31T00:00:00.000Z")), 366);

    // Looks like leap-year (but isn't) tests
    assertEquals(date.dayOfYear(new Date("1900-01-01T00:00:00.000Z")), 1);
    assertEquals(date.dayOfYear(new Date("1900-01-15T00:00:00.000Z")), 15);
    assertEquals(date.dayOfYear(new Date("1900-02-01T00:00:00.000Z")), 32);
    assertEquals(date.dayOfYear(new Date("1900-02-15T00:00:00.000Z")), 46);
    assertEquals(date.dayOfYear(new Date("1900-03-01T00:00:00.000Z")), 60);
    assertEquals(date.dayOfYear(new Date("1900-03-15T00:00:00.000Z")), 74);
    assertEquals(date.dayOfYear(new Date("1900-04-01T00:00:00.000Z")), 91);
    assertEquals(date.dayOfYear(new Date("1900-12-31T00:00:00.000Z")), 365);

    // Year one tests (non-leap)
    assertEquals(date.dayOfYear(new Date("0001-01-01T00:00:00.000Z")), 1);
    assertEquals(date.dayOfYear(new Date("0001-01-15T00:00:00.000Z")), 15);
    assertEquals(date.dayOfYear(new Date("0001-02-01T00:00:00.000Z")), 32);
    assertEquals(date.dayOfYear(new Date("0001-02-15T00:00:00.000Z")), 46);
    assertEquals(date.dayOfYear(new Date("0001-03-01T00:00:00.000Z")), 60);
    assertEquals(date.dayOfYear(new Date("0001-03-15T00:00:00.000Z")), 74);
    assertEquals(date.dayOfYear(new Date("0001-04-01T00:00:00.000Z")), 91);
    assertEquals(date.dayOfYear(new Date("0001-12-31T00:00:00.000Z")), 365);

    // Year minus one tests (non-leap)
    assertEquals(
      date.dayOfYear(new Date("-000001-01-01T00:00:00.000Z")),
      1,
    );
    assertEquals(
      date.dayOfYear(new Date("-000001-01-15T00:00:00.000Z")),
      15,
    );
    assertEquals(
      date.dayOfYear(new Date("-000001-02-01T00:00:00.000Z")),
      32,
    );
    assertEquals(
      date.dayOfYear(new Date("-000001-02-15T00:00:00.000Z")),
      46,
    );
    assertEquals(
      date.dayOfYear(new Date("-000001-03-01T00:00:00.000Z")),
      60,
    );
    assertEquals(
      date.dayOfYear(new Date("-000001-03-15T00:00:00.000Z")),
      74,
    );
    assertEquals(
      date.dayOfYear(new Date("-000001-04-01T00:00:00.000Z")),
      91,
    );
    assertEquals(
      date.dayOfYear(new Date("-000001-12-31T00:00:00.000Z")),
      365,
    );

    // 400 BC tests (leap-year)
    assertEquals(
      date.dayOfYear(new Date("-000400-01-01T00:00:00.000Z")),
      1,
    );
    assertEquals(
      date.dayOfYear(new Date("-000400-01-15T00:00:00.000Z")),
      15,
    );
    assertEquals(
      date.dayOfYear(new Date("-000400-02-01T00:00:00.000Z")),
      32,
    );
    assertEquals(
      date.dayOfYear(new Date("-000400-02-15T00:00:00.000Z")),
      46,
    );
    assertEquals(
      date.dayOfYear(new Date("-000400-03-01T00:00:00.000Z")),
      61,
    );
    assertEquals(
      date.dayOfYear(new Date("-000400-03-15T00:00:00.000Z")),
      75,
    );
    assertEquals(
      date.dayOfYear(new Date("-000400-04-01T00:00:00.000Z")),
      92,
    );
    assertEquals(
      date.dayOfYear(new Date("-000400-12-31T00:00:00.000Z")),
      366,
    );

    // Special Cases

    // Gregorian calendar change (no effect)
    assertEquals(date.dayOfYear(new Date("1582-10-04T03:24:00.000Z")), 277);
    assertEquals(date.dayOfYear(new Date("1582-10-15T03:24:00.000Z")), 288);
  },
});

Deno.test({
  name: "[std/date] weekOfYear",
  fn: () => {
    assertEquals(date.weekOfYear(new Date("2020-01-05T03:00:00.000Z")), 1);
    assertEquals(date.weekOfYear(new Date("2020-06-28T03:00:00.000Z")), 26);

    // iso weeks year starting sunday
    assertEquals(date.weekOfYear(new Date(2012, 0, 1)), 52);
    assertEquals(date.weekOfYear(new Date(2012, 0, 2)), 1);
    assertEquals(date.weekOfYear(new Date(2012, 0, 8)), 1);
    assertEquals(date.weekOfYear(new Date(2012, 0, 9)), 2);
    assertEquals(date.weekOfYear(new Date(2012, 0, 15)), 2);

    // iso weeks year starting monday
    assertEquals(date.weekOfYear(new Date(2007, 0, 1)), 1);
    assertEquals(date.weekOfYear(new Date(2007, 0, 7)), 1);
    assertEquals(date.weekOfYear(new Date(2007, 0, 8)), 2);
    assertEquals(date.weekOfYear(new Date(2007, 0, 14)), 2);
    assertEquals(date.weekOfYear(new Date(2007, 0, 15)), 3);

    // iso weeks year starting tuesday
    assertEquals(date.weekOfYear(new Date(2007, 11, 31)), 1);
    assertEquals(date.weekOfYear(new Date(2008, 0, 1)), 1);
    assertEquals(date.weekOfYear(new Date(2008, 0, 6)), 1);
    assertEquals(date.weekOfYear(new Date(2008, 0, 7)), 2);
    assertEquals(date.weekOfYear(new Date(2008, 0, 13)), 2);
    assertEquals(date.weekOfYear(new Date(2008, 0, 14)), 3);

    // iso weeks year starting wednesday
    assertEquals(date.weekOfYear(new Date(2002, 11, 30)), 1);
    assertEquals(date.weekOfYear(new Date(2003, 0, 1)), 1);
    assertEquals(date.weekOfYear(new Date(2003, 0, 5)), 1);
    assertEquals(date.weekOfYear(new Date(2003, 0, 6)), 2);
    assertEquals(date.weekOfYear(new Date(2003, 0, 12)), 2);
    assertEquals(date.weekOfYear(new Date(2003, 0, 13)), 3);

    // iso weeks year starting thursday
    assertEquals(date.weekOfYear(new Date(2008, 11, 29)), 1);
    assertEquals(date.weekOfYear(new Date(2009, 0, 1)), 1);
    assertEquals(date.weekOfYear(new Date(2009, 0, 4)), 1);
    assertEquals(date.weekOfYear(new Date(2009, 0, 5)), 2);
    assertEquals(date.weekOfYear(new Date(2009, 0, 11)), 2);
    assertEquals(date.weekOfYear(new Date(2009, 0, 13)), 3);

    // iso weeks year starting friday
    assertEquals(date.weekOfYear(new Date(2009, 11, 28)), 53);
    assertEquals(date.weekOfYear(new Date(2010, 0, 1)), 53);
    assertEquals(date.weekOfYear(new Date(2010, 0, 3)), 53);
    assertEquals(date.weekOfYear(new Date(2010, 0, 4)), 1);
    assertEquals(date.weekOfYear(new Date(2010, 0, 10)), 1);
    assertEquals(date.weekOfYear(new Date(2010, 0, 11)), 2);

    // iso weeks year starting saturday
    assertEquals(date.weekOfYear(new Date(2010, 11, 27)), 52);
    assertEquals(date.weekOfYear(new Date(2011, 0, 1)), 52);
    assertEquals(date.weekOfYear(new Date(2011, 0, 2)), 52);
    assertEquals(date.weekOfYear(new Date(2011, 0, 3)), 1);
    assertEquals(date.weekOfYear(new Date(2011, 0, 9)), 1);
    assertEquals(date.weekOfYear(new Date(2011, 0, 10)), 2);
  },
});

Deno.test({
  name: "[std/date] to IMF",
  fn(): void {
    const actual = date.toIMF(new Date(Date.UTC(1994, 3, 5, 15, 32)));
    const expected = "Tue, 05 Apr 1994 15:32:00 GMT";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "[std/date] to IMF 0",
  fn(): void {
    const actual = date.toIMF(new Date(0));
    const expected = "Thu, 01 Jan 1970 00:00:00 GMT";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "[std/date] isLeap",
  fn(): void {
    assert(date.isLeap(1992));
    assert(date.isLeap(2000));
    assert(!date.isLeap(2003));
    assert(!date.isLeap(2007));
  },
});

Deno.test({
  name: "[std/date] difference",
  fn(): void {
    const denoInit = new Date("2018/5/14");
    const denoRelaseV1 = new Date("2020/5/13");
    let difference = date.difference(denoRelaseV1, denoInit, {
      units: ["days", "months", "years"],
    });
    assertEquals(difference.days, 730);
    assertEquals(difference.months, 23);
    assertEquals(difference.years, 1);

    const birth = new Date("1998/2/23 10:10:10");
    const old = new Date("1998/2/23 11:11:11");
    difference = date.difference(birth, old, {
      units: ["milliseconds", "minutes", "seconds", "hours"],
    });
    assertEquals(difference.milliseconds, 3661000);
    assertEquals(difference.seconds, 3661);
    assertEquals(difference.minutes, 61);
    assertEquals(difference.hours, 1);
  },
});

Deno.test({
  name: "[std/date] constants",
  fn(): void {
    assertEquals(date.SECOND, 1e3);
    assertEquals(date.MINUTE, date.SECOND * 60);
    assertEquals(date.HOUR, date.MINUTE * 60);
    assertEquals(date.DAY, date.HOUR * 24);
    assertEquals(date.WEEK, date.DAY * 7);
  },
});
