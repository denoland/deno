// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { getCharset } from "./mod.ts";

Deno.test({
  name: "media-types - getCharset()",
  fn() {
    const fixtures = [
      ["text/plain", "UTF-8"],
      ["text/html", "UTF-8"],
      ["application/foo", undefined],
      ["application/news-checkgroups", "US-ASCII"],
      ["application/news-checkgroups; charset=UTF-8", "UTF-8"],
    ] as const;
    for (const [fixture, expected] of fixtures) {
      assertEquals(getCharset(fixture), expected);
    }
  },
});
