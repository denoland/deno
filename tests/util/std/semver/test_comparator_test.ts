// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { parseComparator } from "./parse_comparator.ts";
import { testComparator } from "./test_comparator.ts";

Deno.test("test", function () {
  const c = parseComparator(">=1.2.3");
  assert(testComparator(parse("1.2.4"), c));
});
