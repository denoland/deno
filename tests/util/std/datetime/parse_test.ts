// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../assert/mod.ts";
import { FakeTime } from "../testing/time.ts";
import { parse } from "./parse.ts";

Deno.test({
  name: "[std/datetime] parse",
  fn: () => {
    assertEquals(
      parse("01-03-2019 16:30", "MM-dd-yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      parse("01.03.2019 16:30", "MM.dd.yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      parse("01.03.2019 16:30", "MM.dd.yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 30),
    );
    assertEquals(
      parse("03-01-2019 16:31", "dd-MM-yyyy HH:mm"),
      new Date(2019, 0, 3, 16, 31),
    );
    assertEquals(
      parse("2019-01-03 16:32", "yyyy-MM-dd HH:mm"),
      new Date(2019, 0, 3, 16, 32),
    );
    assertEquals(
      parse("16:33 01-03-2019", "HH:mm MM-dd-yyyy"),
      new Date(2019, 0, 3, 16, 33),
    );
    assertEquals(
      parse("01-03-2019 16:33:23.123", "MM-dd-yyyy HH:mm:ss.SSS"),
      new Date(2019, 0, 3, 16, 33, 23, 123),
    );
    assertEquals(
      parse("01-03-2019 09:33 PM", "MM-dd-yyyy HH:mm a"),
      new Date(2019, 0, 3, 21, 33),
    );
    assertEquals(
      parse("16:34 03-01-2019", "HH:mm dd-MM-yyyy"),
      new Date(2019, 0, 3, 16, 34),
    );
    assertEquals(
      parse("16:35 2019-01-03", "HH:mm yyyy-MM-dd"),
      new Date(2019, 0, 3, 16, 35),
    );
    assertEquals(
      parse("01-03-2019", "MM-dd-yyyy"),
      new Date(2019, 0, 3),
    );
    assertEquals(
      parse("03-01-2019", "dd-MM-yyyy"),
      new Date(2019, 0, 3),
    );
    assertEquals(
      parse("31-10-2019", "dd-MM-yyyy"),
      new Date(2019, 9, 31),
    );
    assertEquals(
      parse("2019-01-03", "yyyy-MM-dd"),
      new Date(2019, 0, 3),
    );
  },
});

Deno.test("[std/datetime] parse: The date is 2021-12-31", () => {
  const time = new FakeTime("2021-12-31");
  try {
    assertEquals(
      parse("01-01", "MM-dd"),
      new Date(2021, 0, 1),
    );
    assertEquals(
      parse("02-01", "MM-dd"),
      new Date(2021, 1, 1),
    );
    assertEquals(
      parse("03-01", "MM-dd"),
      new Date(2021, 2, 1),
    );
    assertEquals(
      parse("04-01", "MM-dd"),
      new Date(2021, 3, 1),
    );
    assertEquals(
      parse("05-01", "MM-dd"),
      new Date(2021, 4, 1),
    );
    assertEquals(
      parse("06-01", "MM-dd"),
      new Date(2021, 5, 1),
    );
    assertEquals(
      parse("07-01", "MM-dd"),
      new Date(2021, 6, 1),
    );
    assertEquals(
      parse("08-01", "MM-dd"),
      new Date(2021, 7, 1),
    );
    assertEquals(
      parse("09-01", "MM-dd"),
      new Date(2021, 8, 1),
    );
    assertEquals(
      parse("10-01", "MM-dd"),
      new Date(2021, 9, 1),
    );
    assertEquals(
      parse("11-01", "MM-dd"),
      new Date(2021, 10, 1),
    );
    assertEquals(
      parse("12-01", "MM-dd"),
      new Date(2021, 11, 1),
    );

    assertEquals(
      parse("01", "dd"),
      new Date(2021, 11, 1),
    );
    assertEquals(
      parse("15", "dd"),
      new Date(2021, 11, 15),
    );
    assertEquals(
      parse("31", "dd"),
      new Date(2021, 11, 31),
    );
  } finally {
    time.restore();
  }
});

Deno.test({
  name: "[std/datetime] invalidParseDateTimeFormatThrows",
  fn: () => {
    assertThrows(() => {
      parse("2019-01-01 00:00", "x-y-z");
    }, Error);
    assertThrows(() => {
      parse("2019-01-01", "x-y-z");
    }, Error);
  },
});
