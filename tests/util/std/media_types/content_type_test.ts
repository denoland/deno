// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { contentType } from "./content_type.ts";
import { assertEquals } from "../assert/mod.ts";

Deno.test({
  name: "media_types - contentType()",
  fn() {
    const fixtures = [
      [".json", "application/json; charset=UTF-8"],
      ["text/html", "text/html; charset=UTF-8"],
      ["txt", "text/plain; charset=UTF-8"],
      ["text/plain; charset=ISO-8859-1", "text/plain; charset=ISO-8859-1"],
      ["foo", undefined],
      ["file.json", undefined],
      ["application/foo", "application/foo"],
    ] as const;
    for (const [fixture, expected] of fixtures) {
      assertEquals(contentType(fixture), expected);
    }
  },
});

Deno.test({
  name: "media_types - contentType()",
  fn() {
    let _str: string;
    // For well-known content types, the return type is a string.
    // string is assignable to string
    _str = contentType(".json");
    _str = contentType("text/html");
    _str = contentType("txt");

    // @ts-expect-error: string | undefined is not assignable to string
    _str = contentType("text/plain; charset=ISO-8859-1");
    // @ts-expect-error: string | undefined is not assignable to string
    _str = contentType("foo");
    // @ts-expect-error: string | undefined is not assignable to string
    _str = contentType("file.json");
    // @ts-expect-error: string | undefined is not assignable to string
    _str = contentType("application/foo");
  },
});
