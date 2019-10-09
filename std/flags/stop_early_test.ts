// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

// stops parsing on the first non-option when stopEarly is set
test(function stopParsing(): void {
  const argv = parse(["--aaa", "bbb", "ccc", "--ddd"], {
    stopEarly: true
  });

  assertEquals(argv, {
    aaa: "bbb",
    _: ["ccc", "--ddd"]
  });
});
