// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

// stops parsing on the first non-option when stopEarly is set
Deno.test("stopParsing", function (): void {
  const argv = parse(["--aaa", "bbb", "ccc", "--ddd"], {
    stopEarly: true,
  });

  assertEquals(argv, {
    aaa: "bbb",
    _: ["ccc", "--ddd"],
  });
});
