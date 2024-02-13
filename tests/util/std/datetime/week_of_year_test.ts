// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { weekOfYear } from "./week_of_year.ts";

Deno.test({
  name: "[std/datetime] weekOfYear",
  fn: () => {
    assertEquals(weekOfYear(new Date("2020-01-05T03:00:00.000Z")), 1);
    assertEquals(weekOfYear(new Date("2020-06-28T03:00:00.000Z")), 26);

    // iso weeks year starting sunday
    assertEquals(weekOfYear(new Date(2012, 0, 1)), 52);
    assertEquals(weekOfYear(new Date(2012, 0, 2)), 1);
    assertEquals(weekOfYear(new Date(2012, 0, 8)), 1);
    assertEquals(weekOfYear(new Date(2012, 0, 9)), 2);
    assertEquals(weekOfYear(new Date(2012, 0, 15)), 2);

    // iso weeks year starting monday
    assertEquals(weekOfYear(new Date(2007, 0, 1)), 1);
    assertEquals(weekOfYear(new Date(2007, 0, 7)), 1);
    assertEquals(weekOfYear(new Date(2007, 0, 8)), 2);
    assertEquals(weekOfYear(new Date(2007, 0, 14)), 2);
    assertEquals(weekOfYear(new Date(2007, 0, 15)), 3);

    // iso weeks year starting tuesday
    assertEquals(weekOfYear(new Date(2007, 11, 31)), 1);
    assertEquals(weekOfYear(new Date(2008, 0, 1)), 1);
    assertEquals(weekOfYear(new Date(2008, 0, 6)), 1);
    assertEquals(weekOfYear(new Date(2008, 0, 7)), 2);
    assertEquals(weekOfYear(new Date(2008, 0, 13)), 2);
    assertEquals(weekOfYear(new Date(2008, 0, 14)), 3);

    // iso weeks year starting wednesday
    assertEquals(weekOfYear(new Date(2002, 11, 30)), 1);
    assertEquals(weekOfYear(new Date(2003, 0, 1)), 1);
    assertEquals(weekOfYear(new Date(2003, 0, 5)), 1);
    assertEquals(weekOfYear(new Date(2003, 0, 6)), 2);
    assertEquals(weekOfYear(new Date(2003, 0, 12)), 2);
    assertEquals(weekOfYear(new Date(2003, 0, 13)), 3);

    // iso weeks year starting thursday
    assertEquals(weekOfYear(new Date(2008, 11, 29)), 1);
    assertEquals(weekOfYear(new Date(2009, 0, 1)), 1);
    assertEquals(weekOfYear(new Date(2009, 0, 4)), 1);
    assertEquals(weekOfYear(new Date(2009, 0, 5)), 2);
    assertEquals(weekOfYear(new Date(2009, 0, 11)), 2);
    assertEquals(weekOfYear(new Date(2009, 0, 13)), 3);

    // iso weeks year starting friday
    assertEquals(weekOfYear(new Date(2009, 11, 28)), 53);
    assertEquals(weekOfYear(new Date(2010, 0, 1)), 53);
    assertEquals(weekOfYear(new Date(2010, 0, 3)), 53);
    assertEquals(weekOfYear(new Date(2010, 0, 4)), 1);
    assertEquals(weekOfYear(new Date(2010, 0, 10)), 1);
    assertEquals(weekOfYear(new Date(2010, 0, 11)), 2);

    // iso weeks year starting saturday
    assertEquals(weekOfYear(new Date(2010, 11, 27)), 52);
    assertEquals(weekOfYear(new Date(2011, 0, 1)), 52);
    assertEquals(weekOfYear(new Date(2011, 0, 2)), 52);
    assertEquals(weekOfYear(new Date(2011, 0, 3)), 1);
    assertEquals(weekOfYear(new Date(2011, 0, 9)), 1);
    assertEquals(weekOfYear(new Date(2011, 0, 10)), 2);
  },
});
