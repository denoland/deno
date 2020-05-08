// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import * as datetime from "./mod.ts";

Deno.test({
  name: "[std/datetime] parseDate",
  fn: () => {
    assertEquals(
      datetime.parseDateTime("01-03-2019 16:30", "mm-dd-yyyy hh:mm"),
      new Date(2019, 0, 3, 16, 30)
    );
    assertEquals(
      datetime.parseDateTime("03-01-2019 16:31", "dd-mm-yyyy hh:mm"),
      new Date(2019, 0, 3, 16, 31)
    );
    assertEquals(
      datetime.parseDateTime("2019-01-03 16:32", "yyyy-mm-dd hh:mm"),
      new Date(2019, 0, 3, 16, 32)
    );
    assertEquals(
      datetime.parseDateTime("16:33 01-03-2019", "hh:mm mm-dd-yyyy"),
      new Date(2019, 0, 3, 16, 33)
    );
    assertEquals(
      datetime.parseDateTime("16:34 03-01-2019", "hh:mm dd-mm-yyyy"),
      new Date(2019, 0, 3, 16, 34)
    );
    assertEquals(
      datetime.parseDateTime("16:35 2019-01-03", "hh:mm yyyy-mm-dd"),
      new Date(2019, 0, 3, 16, 35)
    );
  },
});

Deno.test({
  name: "[std/datetime] invalidParseDateTimeFormatThrows",
  fn: () => {
    assertThrows(
      (): void => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (datetime as any).parseDateTime("2019-01-01 00:00", "x-y-z");
      },
      Error,
      "Invalid datetime format!"
    );
  },
});

Deno.test({
  name: "[std/datetime] parseDate",
  fn: () => {
    assertEquals(
      datetime.parseDate("01-03-2019", "mm-dd-yyyy"),
      new Date(2019, 0, 3)
    );
    assertEquals(
      datetime.parseDate("03-01-2019", "dd-mm-yyyy"),
      new Date(2019, 0, 3)
    );
    assertEquals(
      datetime.parseDate("2019-01-03", "yyyy-mm-dd"),
      new Date(2019, 0, 3)
    );
  },
});

Deno.test({
  name: "[std/datetime] invalidParseDateFormatThrows",
  fn: () => {
    assertThrows(
      (): void => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (datetime as any).parseDate("2019-01-01", "x-y-z");
      },
      Error,
      "Invalid date format!"
    );
  },
});

Deno.test({
  name: "[std/datetime] DayOfYear",
  fn: () => {
    assertEquals(1, datetime.dayOfYear(new Date("2019-01-01T03:24:00")));
    assertEquals(70, datetime.dayOfYear(new Date("2019-03-11T03:24:00")));
    assertEquals(365, datetime.dayOfYear(new Date("2019-12-31T03:24:00")));
  },
});

Deno.test({
  name: "[std/datetime] currentDayOfYear",
  fn: () => {
    assertEquals(datetime.currentDayOfYear(), datetime.dayOfYear(new Date()));
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
  name: "[std/datetime] Difference",
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
