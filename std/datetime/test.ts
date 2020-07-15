// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import * as datetime from "./mod.ts";

Deno.test({
  name: "[std/datetime] parse",
  fn: () => {
    assertEquals(
      datetime.parse("01-03-2019 16:30", "MM-dd-yyyy hh:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      datetime.parse("01.03.2019 16:30", "MM.dd.yyyy hh:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      datetime.parse("03-01-2019 16:31", "dd-MM-yyyy hh:mm"),
      new Date(2019, 0, 3, 16, 31),
    );
    assertEquals(
      datetime.parse("2019-01-03 16:32", "yyyy-MM-dd hh:mm"),
      new Date(2019, 0, 3, 16, 32),
    );
    assertEquals(
      datetime.parse("16:33 01-03-2019", "hh:mm MM-dd-yyyy"),
      new Date(2019, 0, 3, 16, 33),
    );
    assertEquals(
      datetime.parse("01-03-2019 16:33:23.123", "MM-dd-yyyy hh:mm:ss.SSS"),
      new Date(2019, 0, 3, 16, 33, 23, 123),
    );
    assertEquals(
      datetime.parse("01-03-2019 09:33 PM", "MM-dd-yyyy hh:mm a"),
      new Date(2019, 0, 3, 21, 33),
    );
    assertEquals(
      datetime.parse("16:34 03-01-2019", "hh:mm dd-MM-yyyy"),
      new Date(2019, 0, 3, 16, 34),
    );
    assertEquals(
      datetime.parse("16:35 2019-01-03", "hh:mm yyyy-MM-dd"),
      new Date(2019, 0, 3, 16, 35),
    );
    assertEquals(
      datetime.parse("01-03-2019", "MM-dd-yyyy"),
      new Date(2019, 0, 3),
    );
    assertEquals(
      datetime.parse("03-01-2019", "dd-MM-yyyy"),
      new Date(2019, 0, 3),
    );
    assertEquals(
      datetime.parse("2019-01-03", "yyyy-MM-dd"),
      new Date(2019, 0, 3),
    );
  },
});

Deno.test({
  name: "[std/datetime] invalidParseDateTimeFormatThrows",
  fn: () => {
    assertThrows((): void => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (datetime as any).parse("2019-01-01 00:00", "x-y-z");
    }, Error);
    assertThrows((): void => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (datetime as any).parse("2019-01-01", "x-y-z");
    }, Error);
  },
});

Deno.test({
  name: "[std/datetime] format",
  fn: () => {
    assertEquals(
      "2019-01-01",
      datetime.format(new Date("2019-01-01T03:24:00"), "yyyy-MM-dd"),
    );
    assertEquals(
      "01.01.2019",
      datetime.format(new Date("2019-01-01T03:24:00"), "dd.MM.yyyy"),
    );
    assertEquals(
      "03:24:00",
      datetime.format(new Date("2019-01-01T03:24:00"), "hh:mm:ss"),
    );
    assertEquals(
      "03:24:00.532",
      datetime.format(new Date("2019-01-01T03:24:00.532"), "hh:mm:ss.SSS"),
    );
    assertEquals(
      "03:24:00 AM",
      datetime.format(new Date("2019-01-01T03:24:00"), "hh:mm:ss a"),
    );
    assertEquals(
      "09:24:00 PM",
      datetime.format(new Date("2019-01-01T21:24:00"), "hh:mm:ss a"),
    );
    assertEquals(
      datetime.format(new Date(2019, 0, 20), "'today:' yyyy-MM-dd"),
      "today: 2019-01-20",
    );
  },
});

Deno.test({
  name: "[std/datetime] dayOfYear",
  fn: () => {
    assertEquals(datetime.dayOfYear(new Date("2019-01-01T03:24:00")), 1);
    assertEquals(datetime.dayOfYear(new Date("2019-03-11T03:24:00")), 70);
    assertEquals(datetime.dayOfYear(new Date("2019-12-31T03:24:00")), 365);
  },
});

Deno.test({
  name: "[std/datetime] currentDayOfYear",
  fn: () => {
    assertEquals(datetime.dayOfYear(new Date()), datetime.currentDayOfYear());
  },
});

Deno.test({
  name: "[std/datetime] weekOfYear",
  fn: () => {
    assertEquals(datetime.weekOfYear(new Date("2020-01-05T03:24:00")), 1);
    assertEquals(datetime.weekOfYear(new Date("2020-12-28T03:24:00")), 53); // 53 weeks in 2020
    assertEquals(datetime.weekOfYear(new Date("2020-06-28T03:24:00")), 26);

    // iso weeks year starting sunday
    assertEquals(datetime.weekOfYear(new Date(2012, 0, 1)), 52);
    assertEquals(datetime.weekOfYear(new Date(2012, 0, 2)), 1);
    assertEquals(datetime.weekOfYear(new Date(2012, 0, 8)), 1);
    assertEquals(datetime.weekOfYear(new Date(2012, 0, 9)), 2);
    assertEquals(datetime.weekOfYear(new Date(2012, 0, 15)), 2);

    // iso weeks year starting monday
    assertEquals(datetime.weekOfYear(new Date(2007, 0, 1)), 1);
    assertEquals(datetime.weekOfYear(new Date(2007, 0, 7)), 1);
    assertEquals(datetime.weekOfYear(new Date(2007, 0, 8)), 2);
    assertEquals(datetime.weekOfYear(new Date(2007, 0, 14)), 2);
    assertEquals(datetime.weekOfYear(new Date(2007, 0, 15)), 3);

    // iso weeks year starting tuesday
    assertEquals(datetime.weekOfYear(new Date(2007, 11, 31)), 1);
    assertEquals(datetime.weekOfYear(new Date(2008, 0, 1)), 1);
    assertEquals(datetime.weekOfYear(new Date(2008, 0, 6)), 1);
    assertEquals(datetime.weekOfYear(new Date(2008, 0, 7)), 2);
    assertEquals(datetime.weekOfYear(new Date(2008, 0, 13)), 2);
    assertEquals(datetime.weekOfYear(new Date(2008, 0, 14)), 3);

    // iso weeks year starting wednesday
    assertEquals(datetime.weekOfYear(new Date(2002, 11, 30)), 1);
    assertEquals(datetime.weekOfYear(new Date(2003, 0, 1)), 1);
    assertEquals(datetime.weekOfYear(new Date(2003, 0, 5)), 1);
    assertEquals(datetime.weekOfYear(new Date(2003, 0, 6)), 2);
    assertEquals(datetime.weekOfYear(new Date(2003, 0, 12)), 2);
    assertEquals(datetime.weekOfYear(new Date(2003, 0, 13)), 3);

    // iso weeks year starting thursday
    assertEquals(datetime.weekOfYear(new Date(2008, 11, 29)), 1);
    assertEquals(datetime.weekOfYear(new Date(2009, 0, 1)), 1);
    assertEquals(datetime.weekOfYear(new Date(2009, 0, 4)), 1);
    assertEquals(datetime.weekOfYear(new Date(2009, 0, 5)), 2);
    assertEquals(datetime.weekOfYear(new Date(2009, 0, 11)), 2);
    assertEquals(datetime.weekOfYear(new Date(2009, 0, 13)), 3);

    // iso weeks year starting friday
    assertEquals(datetime.weekOfYear(new Date(2009, 11, 28)), 53);
    assertEquals(datetime.weekOfYear(new Date(2010, 0, 1)), 53);
    assertEquals(datetime.weekOfYear(new Date(2010, 0, 3)), 53);
    assertEquals(datetime.weekOfYear(new Date(2010, 0, 4)), 1);
    assertEquals(datetime.weekOfYear(new Date(2010, 0, 10)), 1);
    assertEquals(datetime.weekOfYear(new Date(2010, 0, 11)), 2);

    // iso weeks year starting saturday
    assertEquals(datetime.weekOfYear(new Date(2010, 11, 27)), 52);
    assertEquals(datetime.weekOfYear(new Date(2011, 0, 1)), 52);
    assertEquals(datetime.weekOfYear(new Date(2011, 0, 2)), 52);
    assertEquals(datetime.weekOfYear(new Date(2011, 0, 3)), 1);
    assertEquals(datetime.weekOfYear(new Date(2011, 0, 9)), 1);
    assertEquals(datetime.weekOfYear(new Date(2011, 0, 10)), 2);
  },
});

Deno.test({
  name: "[std/datetime] to IMF",
  fn(): void {
    const actual = datetime.toIMF(new Date(Date.UTC(1994, 3, 5, 15, 32)));
    const expected = "Tue, 05 Apr 1994 15:32:00 GMT";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "[std/datetime] to IMF 0",
  fn(): void {
    const actual = datetime.toIMF(new Date(0));
    const expected = "Thu, 01 Jan 1970 00:00:00 GMT";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "[std/datetime] isLeap",
  fn(): void {
    assert(datetime.isLeap(1992));
    assert(datetime.isLeap(2000));
    assert(!datetime.isLeap(2003));
    assert(!datetime.isLeap(2007));
  },
});

Deno.test({
  name: "[std/datetime] difference",
  fn(): void {
    const denoInit = new Date("2018/5/14");
    const denoRelaseV1 = new Date("2020/5/13");
    let difference = datetime.difference(denoRelaseV1, denoInit, {
      units: ["days", "months", "years"],
    });
    assertEquals(difference.days, 730);
    assertEquals(difference.months, 23);
    assertEquals(difference.years, 1);

    const birth = new Date("1998/2/23 10:10:10");
    const old = new Date("1998/2/23 11:11:11");
    difference = datetime.difference(birth, old, {
      units: ["miliseconds", "minutes", "seconds", "hours"],
    });
    assertEquals(difference.miliseconds, 3661000);
    assertEquals(difference.seconds, 3661);
    assertEquals(difference.minutes, 61);
    assertEquals(difference.hours, 1);
  },
});
