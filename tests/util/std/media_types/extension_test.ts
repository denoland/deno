// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { extension } from "./mod.ts";

Deno.test({
  name: "media_types - extension()",
  fn() {
    const fixtures: [string, string | undefined][] = [
      ["image/gif", "gif"],
      ["application/javascript", "js"],
      ["text/html; charset=UTF-8", "html"],
      ["application/foo", undefined],
    ];
    for (const [fixture, expected] of fixtures) {
      assertEquals(extension(fixture), expected);
    }
  },
});
