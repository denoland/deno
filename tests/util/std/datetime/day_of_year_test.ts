// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { dayOfYear, dayOfYearUtc } from "./day_of_year.ts";

Deno.test({
  name: "[std/datetime] dayOfYearUtc",
  fn: () => {
    // from https://golang.org/src/time/time_test.go
    // Test YearDay in several different scenarios
    // and corner cases
    // Non-leap-year tests
    assertEquals(dayOfYearUtc(new Date("2007-01-01T00:00:00.000Z")), 1);
    assertEquals(dayOfYearUtc(new Date("2007-01-15T00:00:00.000Z")), 15);
    assertEquals(dayOfYearUtc(new Date("2007-02-01T00:00:00.000Z")), 32);
    assertEquals(dayOfYearUtc(new Date("2007-02-15T00:00:00.000Z")), 46);
    assertEquals(dayOfYearUtc(new Date("2007-03-01T00:00:00.000Z")), 60);
    assertEquals(dayOfYearUtc(new Date("2007-03-15T00:00:00.000Z")), 74);
    assertEquals(dayOfYearUtc(new Date("2007-04-01T00:00:00.000Z")), 91);
    assertEquals(dayOfYearUtc(new Date("2007-12-31T00:00:00.000Z")), 365);

    assertEquals(dayOfYearUtc(new Date("2007-01-01T00:00:00.000Z")), 1);
    assertEquals(
      dayOfYearUtc(new Date("2007-02-01T00:00:00.000Z")),
      31 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-03-01T00:00:00.000Z")),
      31 + 28 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-03-24T00:00:00.000Z")),
      31 + 28 + 24,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-03-25T00:00:00.000Z")),
      31 + 28 + 25,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-04-01T00:00:00.000Z")),
      31 + 28 + 31 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-05-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-06-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-07-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-08-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-09-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-10-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-10-27T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 27,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-10-28T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 28,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-11-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 1,
    );
    assertEquals(
      dayOfYearUtc(new Date("2007-12-01T00:00:00.000Z")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30 + 1,
    );

    // Leap-year tests
    assertEquals(dayOfYearUtc(new Date("2008-01-01T00:00:00.000Z")), 1);
    assertEquals(dayOfYearUtc(new Date("2008-01-15T00:00:00.000Z")), 15);
    assertEquals(dayOfYearUtc(new Date("2008-02-01T00:00:00.000Z")), 32);
    assertEquals(dayOfYearUtc(new Date("2008-02-15T00:00:00.000Z")), 46);
    assertEquals(dayOfYearUtc(new Date("2008-03-01T00:00:00.000Z")), 61);
    assertEquals(dayOfYearUtc(new Date("2008-03-15T00:00:00.000Z")), 75);
    assertEquals(dayOfYearUtc(new Date("2008-04-01T00:00:00.000Z")), 92);
    assertEquals(dayOfYearUtc(new Date("2008-12-31T00:00:00.000Z")), 366);

    // Looks like leap-year (but isn't) tests
    assertEquals(dayOfYearUtc(new Date("1900-01-01T00:00:00.000Z")), 1);
    assertEquals(dayOfYearUtc(new Date("1900-01-15T00:00:00.000Z")), 15);
    assertEquals(dayOfYearUtc(new Date("1900-02-01T00:00:00.000Z")), 32);
    assertEquals(dayOfYearUtc(new Date("1900-02-15T00:00:00.000Z")), 46);
    assertEquals(dayOfYearUtc(new Date("1900-03-01T00:00:00.000Z")), 60);
    assertEquals(dayOfYearUtc(new Date("1900-03-15T00:00:00.000Z")), 74);
    assertEquals(dayOfYearUtc(new Date("1900-04-01T00:00:00.000Z")), 91);
    assertEquals(dayOfYearUtc(new Date("1900-12-31T00:00:00.000Z")), 365);

    // Year one tests (non-leap)
    assertEquals(dayOfYearUtc(new Date("0001-01-01T00:00:00.000Z")), 1);
    assertEquals(dayOfYearUtc(new Date("0001-01-15T00:00:00.000Z")), 15);
    assertEquals(dayOfYearUtc(new Date("0001-02-01T00:00:00.000Z")), 32);
    assertEquals(dayOfYearUtc(new Date("0001-02-15T00:00:00.000Z")), 46);
    assertEquals(dayOfYearUtc(new Date("0001-03-01T00:00:00.000Z")), 60);
    assertEquals(dayOfYearUtc(new Date("0001-03-15T00:00:00.000Z")), 74);
    assertEquals(dayOfYearUtc(new Date("0001-04-01T00:00:00.000Z")), 91);
    assertEquals(dayOfYearUtc(new Date("0001-12-31T00:00:00.000Z")), 365);

    // Year minus one tests (non-leap)
    assertEquals(
      dayOfYearUtc(new Date("-000001-01-01T00:00:00.000Z")),
      1,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000001-01-15T00:00:00.000Z")),
      15,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000001-02-01T00:00:00.000Z")),
      32,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000001-02-15T00:00:00.000Z")),
      46,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000001-03-01T00:00:00.000Z")),
      60,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000001-03-15T00:00:00.000Z")),
      74,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000001-04-01T00:00:00.000Z")),
      91,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000001-12-31T00:00:00.000Z")),
      365,
    );

    // 400 BC tests (leap-year)
    assertEquals(
      dayOfYearUtc(new Date("-000400-01-01T00:00:00.000Z")),
      1,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000400-01-15T00:00:00.000Z")),
      15,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000400-02-01T00:00:00.000Z")),
      32,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000400-02-15T00:00:00.000Z")),
      46,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000400-03-01T00:00:00.000Z")),
      61,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000400-03-15T00:00:00.000Z")),
      75,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000400-04-01T00:00:00.000Z")),
      92,
    );
    assertEquals(
      dayOfYearUtc(new Date("-000400-12-31T00:00:00.000Z")),
      366,
    );

    // Special Cases

    // Gregorian calendar change (no effect)
    assertEquals(dayOfYearUtc(new Date("1582-10-04T03:24:00.000Z")), 277);
    assertEquals(dayOfYearUtc(new Date("1582-10-15T03:24:00.000Z")), 288);
  },
});

Deno.test({
  name: "[std/datetime] dayOfYear",
  fn: () => {
    // from https://golang.org/src/time/time_test.go
    // Test YearDay in several different scenarios
    // and corner cases
    // Non-leap-year tests
    assertEquals(dayOfYear(new Date("2007-01-01T00:00:00.000")), 1);
    assertEquals(dayOfYear(new Date("2007-01-15T00:00:00.000")), 15);
    assertEquals(dayOfYear(new Date("2007-02-01T00:00:00.000")), 32);
    assertEquals(dayOfYear(new Date("2007-02-15T00:00:00.000")), 46);
    assertEquals(dayOfYear(new Date("2007-03-01T00:00:00.000")), 60);
    assertEquals(dayOfYear(new Date("2007-03-15T00:00:00.000")), 74);
    assertEquals(dayOfYear(new Date("2007-04-01T00:00:00.000")), 91);
    assertEquals(dayOfYear(new Date("2007-12-31T00:00:00.000")), 365);

    assertEquals(dayOfYear(new Date("2007-01-01T00:00:00.000")), 1);
    assertEquals(
      dayOfYear(new Date("2007-02-01T00:00:00.000")),
      31 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-03-01T00:00:00.000")),
      31 + 28 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-03-24T00:00:00.000")),
      31 + 28 + 24,
    );
    assertEquals(
      dayOfYear(new Date("2007-03-25T00:00:00.000")),
      31 + 28 + 25,
    );
    assertEquals(
      dayOfYear(new Date("2007-04-01T00:00:00.000")),
      31 + 28 + 31 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-05-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-06-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-07-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-08-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-09-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-10-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-10-27T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 27,
    );
    assertEquals(
      dayOfYear(new Date("2007-10-28T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 28,
    );
    assertEquals(
      dayOfYear(new Date("2007-11-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 1,
    );
    assertEquals(
      dayOfYear(new Date("2007-12-01T00:00:00.000")),
      31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30 + 1,
    );

    // Leap-year tests
    assertEquals(dayOfYear(new Date("2008-01-01T00:00:00.000")), 1);
    assertEquals(dayOfYear(new Date("2008-01-15T00:00:00.000")), 15);
    assertEquals(dayOfYear(new Date("2008-02-01T00:00:00.000")), 32);
    assertEquals(dayOfYear(new Date("2008-02-15T00:00:00.000")), 46);
    assertEquals(dayOfYear(new Date("2008-03-01T00:00:00.000")), 61);
    assertEquals(dayOfYear(new Date("2008-03-15T00:00:00.000")), 75);
    assertEquals(dayOfYear(new Date("2008-04-01T00:00:00.000")), 92);
    assertEquals(dayOfYear(new Date("2008-12-31T00:00:00.000")), 366);

    // Looks like leap-year (but isn't) tests
    assertEquals(dayOfYear(new Date("1900-01-01T00:00:00.000")), 1);
    assertEquals(dayOfYear(new Date("1900-01-15T00:00:00.000")), 15);
    assertEquals(dayOfYear(new Date("1900-02-01T00:00:00.000")), 32);
    assertEquals(dayOfYear(new Date("1900-02-15T00:00:00.000")), 46);
    assertEquals(dayOfYear(new Date("1900-03-01T00:00:00.000")), 60);
    assertEquals(dayOfYear(new Date("1900-03-15T00:00:00.000")), 74);
    assertEquals(dayOfYear(new Date("1900-04-01T00:00:00.000")), 91);
    assertEquals(dayOfYear(new Date("1900-12-31T00:00:00.000")), 365);

    // Year one tests (non-leap)
    assertEquals(dayOfYear(new Date("0001-01-01T00:00:00.000")), 1);
    assertEquals(dayOfYear(new Date("0001-01-15T00:00:00.000")), 15);
    assertEquals(dayOfYear(new Date("0001-02-01T00:00:00.000")), 32);
    assertEquals(dayOfYear(new Date("0001-02-15T00:00:00.000")), 46);
    assertEquals(dayOfYear(new Date("0001-03-01T00:00:00.000")), 60);
    assertEquals(dayOfYear(new Date("0001-03-15T00:00:00.000")), 74);
    assertEquals(dayOfYear(new Date("0001-04-01T00:00:00.000")), 91);
    assertEquals(dayOfYear(new Date("0001-12-31T00:00:00.000")), 365);

    // Year minus one tests (non-leap)
    assertEquals(
      dayOfYear(new Date("-000001-01-01T00:00:00.000")),
      1,
    );
    assertEquals(
      dayOfYear(new Date("-000001-01-15T00:00:00.000")),
      15,
    );
    assertEquals(
      dayOfYear(new Date("-000001-02-01T00:00:00.000")),
      32,
    );
    assertEquals(
      dayOfYear(new Date("-000001-02-15T00:00:00.000")),
      46,
    );
    assertEquals(
      dayOfYear(new Date("-000001-03-01T00:00:00.000")),
      60,
    );
    assertEquals(
      dayOfYear(new Date("-000001-03-15T00:00:00.000")),
      74,
    );
    assertEquals(
      dayOfYear(new Date("-000001-04-01T00:00:00.000")),
      91,
    );
    assertEquals(
      dayOfYear(new Date("-000001-12-31T00:00:00.000")),
      365,
    );

    // 400 BC tests (leap-year)
    assertEquals(
      dayOfYear(new Date("-000400-01-01T00:00:00.000")),
      1,
    );
    assertEquals(
      dayOfYear(new Date("-000400-01-15T00:00:00.000")),
      15,
    );
    assertEquals(
      dayOfYear(new Date("-000400-02-01T00:00:00.000")),
      32,
    );
    assertEquals(
      dayOfYear(new Date("-000400-02-15T00:00:00.000")),
      46,
    );
    assertEquals(
      dayOfYear(new Date("-000400-03-01T00:00:00.000")),
      61,
    );
    assertEquals(
      dayOfYear(new Date("-000400-03-15T00:00:00.000")),
      75,
    );
    assertEquals(
      dayOfYear(new Date("-000400-04-01T00:00:00.000")),
      92,
    );
    assertEquals(
      dayOfYear(new Date("-000400-12-31T00:00:00.000")),
      366,
    );

    // Special Cases

    // Gregorian calendar change (no effect)
    assertEquals(dayOfYear(new Date("1582-10-04T03:24:00.000")), 277);
    assertEquals(dayOfYear(new Date("1582-10-15T03:24:00.000")), 288);
  },
});
