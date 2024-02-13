// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { common } from "./mod.ts";

Deno.test({
  name: "common() returns shared path",
  fn() {
    const actual = common(
      [
        "file://deno/cli/js/deno.ts",
        "file://deno/std/path/mod.ts",
        "file://deno/cli/js/main.ts",
      ],
      "/",
    );
    assertEquals(actual, "file://deno/");
  },
});

Deno.test({
  name: "common() returns empty string if no shared path is present",
  fn() {
    const actual = common(
      ["file://deno/cli/js/deno.ts", "https://deno.land/std/path/mod.ts"],
      "/",
    );
    assertEquals(actual, "");
  },
});

Deno.test({
  name: "common() checks windows separator",
  fn() {
    const actual = common(
      [
        "c:\\deno\\cli\\js\\deno.ts",
        "c:\\deno\\std\\path\\mod.ts",
        "c:\\deno\\cli\\js\\main.ts",
      ],
      "\\",
    );
    assertEquals(actual, "c:\\deno\\");
  },
});
