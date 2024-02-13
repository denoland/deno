// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { typeByExtension } from "./mod.ts";

Deno.test({
  name: "media_types - typeByExtension",
  fn() {
    const fixtures = [
      ["js", "application/javascript"],
      [".js", "application/javascript"],
      ["Js", "application/javascript"],
      ["html", "text/html"],
      [".html", "text/html"],
      [".HTML", "text/html"],
      ["file.json", undefined],
      ["foo", undefined],
      [".foo", undefined],
    ] as const;
    for (const [fixture, expected] of fixtures) {
      assertEquals(typeByExtension(fixture), expected);
    }
  },
});
