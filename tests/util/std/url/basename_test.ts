// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import * as url from "./mod.ts";

const TESTSUITE: [[string | URL, string?], string][] = [
  [["https://deno.land/std/assert/mod.ts"], "mod.ts"],
  [[new URL("https://deno.land/std/assert/mod.ts")], "mod.ts"],
  [[new URL("https://deno.land/std/assert/mod.ts"), ".ts"], "mod"],
  [[new URL("https://deno.land/std/assert/mod.ts?foo=bar")], "mod.ts"],
  [[new URL("https://deno.land/std/assert/mod.ts#header")], "mod.ts"],
  [[new URL("https://deno.land///")], "deno.land"],
];

Deno.test("basename", function () {
  for (const [[test_url, suffix], expected] of TESTSUITE) {
    assertEquals(url.basename(test_url, suffix), expected);
  }
});
