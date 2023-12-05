// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
<<<<<<< HEAD
import { assertEquals } from "../../../test_util/std/assert/mod.ts";
=======
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
>>>>>>> 172e5f0a0 (1.38.5 (#21469))
import { parse, stringify } from "node:querystring";

Deno.test({
  name: "stringify",
  fn() {
    assertEquals(
      stringify({
        a: "hello",
        b: 5,
        c: true,
        d: ["foo", "bar"],
      }),
      "a=hello&b=5&c=true&d=foo&d=bar",
    );
  },
});

Deno.test({
  name: "parse",
  fn() {
    assertEquals(parse("a=hello&b=5&c=true&d=foo&d=bar"), {
      a: "hello",
      b: "5",
      c: "true",
      d: ["foo", "bar"],
    });
  },
});
