// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";

import { parseDate } from "./date.ts";

Deno.test("[mail/date] parseDate", function (): void {
  assertEquals(
    parseDate("Sun, 25 Sep 2016 18:36:33 -0400"),
    1474842993n,
  );
  assertEquals(
    parseDate("Fri, 01 Jan 2100 11:12:13 +0000"),
    4102485133n,
  );
  assertEquals(
    parseDate("Fri, 31 Dec 2100 00:00:00 +0000"),
    4133894400n,
  );
  assertEquals(
    parseDate("Fri, 31 Dec 2399 00:00:00 +0000"),
    13569379200n,
  );
  assertEquals(
    parseDate("Fri, 31 Dec 2400 00:00:00 +0000"),
    13601001600n,
  );
  assertEquals(parseDate("17 Sep 2016 16:05:38 -1000"), 1474164338n);
  assertEquals(
    parseDate("Fri, 30 Nov 2012 20:57:23 GMT"),
    1354309043n,
  );
});
