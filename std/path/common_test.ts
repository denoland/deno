// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { common } from "./mod.ts";

Deno.test({
  name: "path - common - basic usage",
  fn() {
    const actual = common(
      [
        "file://deno/cli/js/deno.ts",
        "file://deno/std/path/mod.ts",
        "file://deno/cli/js/main.ts",
      ],
      "/"
    );
    assertEquals(actual, "file://deno/");
  },
});

Deno.test({
  name: "path - common - no shared",
  fn() {
    const actual = common(
      ["file://deno/cli/js/deno.ts", "https://deno.land/std/path/mod.ts"],
      "/"
    );
    assertEquals(actual, "");
  },
});

Deno.test({
  name: "path - common - windows sep",
  fn() {
    const actual = common(
      [
        "c:\\deno\\cli\\js\\deno.ts",
        "c:\\deno\\std\\path\\mod.ts",
        "c:\\deno\\cli\\js\\main.ts",
      ],
      "\\"
    );
    assertEquals(actual, "c:\\deno\\");
  },
});
