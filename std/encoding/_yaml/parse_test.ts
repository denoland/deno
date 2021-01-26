// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { parse, parseAll } from "./parse.ts";
import { assertEquals } from "../../testing/asserts.ts";

Deno.test({
  name: "`parse` parses single document yaml string",
  fn(): void {
    const yaml = `
      test: toto
      foo:
        bar: True
        baz: 1
        qux: ~
    `;

    const expected = { test: "toto", foo: { bar: true, baz: 1, qux: null } };

    assertEquals(parse(yaml), expected);
  },
});

Deno.test({
  name: "`parseAll` parses the yaml string with multiple documents",
  fn(): void {
    const yaml = `
---
id: 1
name: Alice
---
id: 2
name: Bob
---
id: 3
name: Eve
    `;
    const expected = [
      {
        id: 1,
        name: "Alice",
      },
      {
        id: 2,
        name: "Bob",
      },
      {
        id: 3,
        name: "Eve",
      },
    ];
    assertEquals(parseAll(yaml), expected);
  },
});
