// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/mod.ts";
import { isLeap, isUtcLeap } from "./is_leap.ts";

Deno.test({
  name: "[std/datetime] isLeap",
  fn() {
    assert(isLeap(1992));
    assert(isLeap(2000));
    assert(!isLeap(2003));
    assert(!isLeap(2007));
    assert(!isLeap(new Date("1970-01-02")));
    assert(isLeap(new Date("1972-01-02")));
    assert(isLeap(new Date("2000-01-02")));
    assert(!isLeap(new Date("2100-01-02")));
  },
});

Deno.test({
  name: "[std/datetime] isUtcLeap",
  fn() {
    assert(isUtcLeap(1992));
    assert(isUtcLeap(2000));
    assert(!isUtcLeap(2003));
    assert(!isUtcLeap(2007));

    // Date assumes the string given is UTC by default
    assert(!isLeap(new Date("1970-01-01")));
    assert(isUtcLeap(new Date("1972-01-01")));
    assert(isUtcLeap(new Date("2000-01-01")));
    assert(!isUtcLeap(new Date("2100-01-01")));

    // Bookends of a leap year
    assert(isUtcLeap(new Date("January 1, 2000 00:00:00 GMT+00:00")));
    assert(isUtcLeap(new Date("December 31, 2000 23:59:59 GMT+00:00")));

    // Edge cases of a UTC leap year from different time zones
    assert(!isUtcLeap(new Date("January 1, 2000 00:00:00 GMT+01:00")));
    assert(!isUtcLeap(new Date("December 31, 2000 23:59:59 GMT-01:00")));
    assert(isUtcLeap(new Date("January 1, 2001 00:00:00 GMT+01:00")));
    assert(isUtcLeap(new Date("December 31, 1999 23:59:59 GMT-01:00")));
  },
});
