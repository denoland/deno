// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import * as url from "./mod.ts";

const TESTSUITE: [[string | URL, ...string[]], URL][] = [
  [
    ["https://deno.land", "std", "assert", "mod.ts"],
    new URL("https://deno.land/std/assert/mod.ts"),
  ],
  [
    [new URL("https://deno.land"), "std", "assert", "mod.ts"],
    new URL("https://deno.land/std/assert/mod.ts"),
  ],
  [
    [new URL("https:///deno.land//std//"), "/", "/assert/", "//mod.ts"],
    new URL("https://deno.land/std/assert/mod.ts"),
  ],
  [
    ["https://deno.land///", "/"],
    new URL("https://deno.land/"),
  ],
];

Deno.test("join", function () {
  for (const [[test_url, ...paths], expected] of TESTSUITE) {
    assertEquals(url.join(test_url, ...paths), expected);
  }
});
